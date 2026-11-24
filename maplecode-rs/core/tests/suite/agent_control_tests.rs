//! Verifies host controller selection and routing through existing runtime entry points.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use core_test_support::ThreadIdle;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_maplecode::TestMapleCode;
use core_test_support::test_maplecode::test_maplecode;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use futures::future::BoxFuture;
use maplecode_core::AgentConfigUpdate;
use maplecode_core::AgentControl;
use maplecode_core::AgentExecutionGuard;
use maplecode_core::AgentInfo;
use maplecode_core::AgentRef;
use maplecode_core::AgentTarget;
use maplecode_core::AgentTurnOutcome;
use maplecode_core::AgentTurnOutcomeReporter;
use maplecode_core::DeliveryReceipt;
use maplecode_core::ForkSnapshot;
use maplecode_core::GuardianRootSnapshot;
use maplecode_core::InheritedSessionId;
use maplecode_core::LiveAgent;
use maplecode_core::PendingAgentMessage;
use maplecode_core::RolloutBudgetReminder;
use maplecode_core::SendRequest;
use maplecode_core::SpawnRequest;
use maplecode_core::StartThreadOptions;
use maplecode_core::ThreadConfigSnapshot;
use maplecode_core::TurnInput;
use maplecode_core::TurnInputRequest;
use maplecode_core::TurnInputSubmission;
use maplecode_core::TurnStartOptions;
use maplecode_core::config::Config;
use maplecode_extension_api::ExtensionFuture;
use maplecode_extension_api::ExtensionRegistryBuilder;
use maplecode_extension_api::ThreadLifecycleContributor;
use maplecode_extension_api::ThreadStartInput;
use maplecode_extension_api::ThreadStopInput;
use maplecode_extension_api::TurnLifecycleContributor;
use maplecode_extension_api::TurnStopInput;
use maplecode_features::Feature;
use maplecode_history::InitialHistory;
use maplecode_history::ResumedHistory;
use maplecode_history::RolloutItem;
use maplecode_message_board::PostMetadata;
use maplecode_protocol::AgentPath;
use maplecode_protocol::SessionId;
use maplecode_protocol::ThreadId;
use maplecode_protocol::error::MapleCodeErr;
use maplecode_protocol::error::MapleCodeErrorDetails;
use maplecode_protocol::error::Result as MapleCodeResult;
use maplecode_protocol::protocol::AgentStatus;
use maplecode_protocol::protocol::InterAgentCommunication;
use maplecode_protocol::protocol::MultiAgentVersion;
use maplecode_protocol::protocol::Op;
use maplecode_protocol::protocol::SessionSource;
use maplecode_protocol::protocol::SubAgentSource;
use maplecode_protocol::protocol::ThreadHistoryMode;
use maplecode_protocol::protocol::ThreadSource;
use maplecode_protocol::protocol::TokenUsage;
use maplecode_protocol::user_input::UserInput;
use maplecode_rollout_trace::ThreadTraceContext;
use maplecode_thread_store::LoadThreadHistoryParams;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use wiremock::MockServer;

pub(super) struct HostRuntime;

impl maplecode_core::AgentTurnAdmission for HostRuntime {
    fn admit_turn(
        &self,
        _: maplecode_core::AgentTurnAdmissionRequest,
    ) -> BoxFuture<'_, MapleCodeResult<AgentExecutionGuard>> {
        Box::pin(async { Ok(AgentExecutionGuard::new(())) })
    }
}

pub(super) struct HostSessionReader {
    root: ThreadId,
    pub(super) children: Mutex<Option<Vec<AgentRef>>>,
    pub(super) children_reads: AtomicUsize,
}

impl maplecode_core::AgentSessionReader for HostSessionReader {
    fn root(&self) -> BoxFuture<'_, MapleCodeResult<AgentRef>> {
        Box::pin(async {
            Ok(AgentRef {
                thread_id: self.root,
                path: AgentPath::root(),
            })
        })
    }

    fn parent(&self) -> BoxFuture<'_, MapleCodeResult<Option<AgentRef>>> {
        Box::pin(async { Ok(None) })
    }

    fn children(&self) -> BoxFuture<'_, MapleCodeResult<Vec<AgentRef>>> {
        Box::pin(async {
            self.children_reads.fetch_add(1, Ordering::Relaxed);
            self.children
                .lock()
                .expect("host roster")
                .clone()
                .ok_or_else(|| {
                    MapleCodeErr::InvalidRequest("host agent session unavailable".into())
                })
        })
    }
}

pub(super) fn install_host_runtime(data: &maplecode_extension_api::ExtensionData) {
    data.insert(maplecode_core::AgentRuntimeOwnership::Host);
    let reader = data.get_or_init(|| HostSessionReader {
        root: ThreadId::from_string(data.level_id()).expect("test thread identity"),
        children: Mutex::new(Some(Vec::new())),
        children_reads: AtomicUsize::new(0),
    });
    data.get_or_init(|| reader as Arc<dyn maplecode_core::AgentSessionReader>);
    data.get_or_init(|| Arc::new(HostRuntime) as Arc<dyn maplecode_core::AgentTurnAdmission>);
}

impl maplecode_extension_api::ThreadLifecycleContributor<maplecode_core::config::Config>
    for HostRuntime
{
    fn on_thread_start<'a>(
        &'a self,
        input: maplecode_extension_api::ThreadStartInput<'a, maplecode_core::config::Config>,
    ) -> maplecode_extension_api::ExtensionFuture<'a, ()> {
        install_host_runtime(input.thread_store);
        Box::pin(async {})
    }
}

pub(super) struct TestAgentControl {
    thread_id: ThreadId,
    service_tier: Mutex<Option<String>>,
    mail: Mutex<Vec<PendingAgentMessage>>,
    mailbox_state: tokio::sync::watch::Sender<bool>,
    agents: Mutex<HashMap<String, ThreadId>>,
}

impl TestAgentControl {
    pub(super) fn new(thread_id: ThreadId) -> Self {
        Self {
            thread_id,
            service_tier: Mutex::default(),
            mail: Mutex::default(),
            mailbox_state: tokio::sync::watch::channel(/*init*/ false).0,
            agents: Mutex::new(HashMap::from([("/root".into(), thread_id)])),
        }
    }
}

impl AgentControl for TestAgentControl {
    fn identity(&self) -> SessionId {
        SessionId::from(self.thread_id)
    }

    fn resolve<'a>(
        &'a self,
        _caller: ThreadId,
        _parent: Option<ThreadId>,
        _source: &'a SessionSource,
        target: &'a str,
    ) -> BoxFuture<'a, MapleCodeResult<ThreadId>> {
        Box::pin(async move {
            if let Some(id) = self.agents.lock().expect("agents lock").get(target) {
                return Ok(*id);
            }
            ThreadId::from_string(target)
                .map_err(|error| MapleCodeErr::InvalidRequest(error.to_string()))
        })
    }

    fn spawn(
        &self,
        request: SpawnRequest,
    ) -> BoxFuture<'_, MapleCodeResult<(LiveAgent, ThreadConfigSnapshot)>> {
        assert_eq!(request.caller, self.thread_id);
        Box::pin(async {
            Err(MapleCodeErr::InvalidRequest(
                "host spawn rejection".to_string(),
            ))
        })
    }

    fn send(&self, request: SendRequest) -> BoxFuture<'_, MapleCodeResult<DeliveryReceipt>> {
        assert_eq!(request.caller, self.thread_id);
        Box::pin(async {
            Err(MapleCodeErr::InvalidRequest(
                "host send rejection".to_string(),
            ))
        })
    }

    fn take_mailbox(&self, _agent: ThreadId) -> Vec<PendingAgentMessage> {
        let messages = std::mem::take(&mut *self.mail.lock().expect("mail lock"));
        self.mailbox_state.send_if_modified(std::mem::take);
        messages
    }

    fn watch_mailbox(&self, _agent: ThreadId) -> tokio::sync::watch::Receiver<bool> {
        self.mailbox_state.subscribe()
    }

    fn ensure_child_loaded(
        &self,
        parent: ThreadId,
        child: ThreadId,
    ) -> BoxFuture<'_, MapleCodeResult<()>> {
        assert_eq!(parent, self.thread_id);
        Box::pin(async move {
            Err(MapleCodeErr::InvalidRequest(format!(
                "host resume rejection for {child}"
            )))
        })
    }

    fn interrupt(
        &self,
        _caller: ThreadId,
        _target: AgentTarget,
        _version: MultiAgentVersion,
    ) -> BoxFuture<'_, MapleCodeResult<AgentInfo>> {
        panic!("unexpected agent interrupt")
    }

    fn list<'a>(
        &'a self,
        caller: ThreadId,
        _parent: Option<ThreadId>,
        _source: &'a SessionSource,
        _path_prefix: Option<&'a str>,
    ) -> BoxFuture<'a, MapleCodeResult<Vec<LiveAgent>>> {
        assert_eq!(caller, self.thread_id);
        Box::pin(async {
            Err(MapleCodeErr::InvalidRequest(
                "host list rejection".to_string(),
            ))
        })
    }

    fn child_agent_paths(&self, _parent: ThreadId) -> BoxFuture<'_, Vec<AgentPath>> {
        Box::pin(async { Vec::new() })
    }

    fn check_turn_admission(
        &self,
        _version: MultiAgentVersion,
        _source: &SessionSource,
    ) -> MapleCodeResult<()> {
        Ok(())
    }

    fn admit_turn(
        &self,
        _version: MultiAgentVersion,
        _source: &SessionSource,
    ) -> Option<AgentExecutionGuard> {
        None
    }

    fn record_usage(&self, _usage: TokenUsage) -> BoxFuture<'_, MapleCodeResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn turn_finished<'a>(
        &'a self,
        _outcome: AgentTurnOutcome,
        _trace: &'a ThreadTraceContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }

    fn service_tier(&self) -> Option<String> {
        self.service_tier.lock().expect("service tier lock").clone()
    }

    fn propagate_config_update(&self, update: AgentConfigUpdate) {
        let AgentConfigUpdate::ServiceTier(tier) = update;
        *self.service_tier.lock().expect("service tier lock") = tier;
    }

    fn get_guardian_package(
        &self,
        _agent: ThreadId,
    ) -> BoxFuture<'_, Option<GuardianRootSnapshot>> {
        Box::pin(async { None })
    }

    fn pending_budget_reminder<'a>(
        &'a self,
        _agent: ThreadId,
        _window: &'a str,
    ) -> BoxFuture<'a, Option<RolloutBudgetReminder>> {
        Box::pin(async { None })
    }

    fn mark_budget_reminder_delivered<'a>(
        &'a self,
        _agent: ThreadId,
        _window: &'a str,
        _reminder: RolloutBudgetReminder,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }
}

async fn test_with_host_control(
    server: &MockServer,
    extra_features: &[Feature],
) -> anyhow::Result<(TestMapleCode, Arc<TestAgentControl>)> {
    let thread_id = ThreadId::new();
    let controller = Arc::new(TestAgentControl::new(thread_id));
    let extra_features = extra_features.to_vec();
    let test = test_maplecode()
        .with_config(move |config| {
            for feature in [
                Feature::Collab,
                Feature::MultiAgentV2,
                Feature::AgentMessageBoard,
            ]
            .into_iter()
            .chain(extra_features.iter().copied())
            {
                config
                    .features
                    .enable(feature)
                    .expect("enable collaboration feature");
            }
            config.service_tier = Some("priority".to_string());
        })
        .with_thread_manager({
            let controller = Arc::clone(&controller);
            move |manager| {
                manager
                    .with_thread_id_generator(move || thread_id)
                    .with_agent_control_factory(move |_| {
                        let control: Arc<dyn AgentControl> = controller.clone();
                        async move { Ok(control) }
                    })
            }
        })
        .build_with_auto_env(server)
        .await?;
    Ok((test, controller))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_threads_preserve_lineage_settings_and_resume_routing() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let (test, controller) = test_with_host_control(&server, &[]).await?;
    let root_id = test.session_configured.thread_id;
    let mut child_config = test.config.clone();
    child_config.service_tier = None;
    let child = test
        .thread_manager
        .start_thread(StartThreadOptions {
            reserved_thread_id: Some(ThreadId::new()),
            environments: Some(vec![test.executor_environment().request()]),
            session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: root_id,
                depth: 1,
                agent_path: Some(AgentPath::root().join("worker").expect("valid child path")),
                agent_nickname: None,
                agent_role: None,
            })),
            ..StartThreadOptions::new(child_config.clone())
        })
        .await?;
    assert_eq!(
        (
            child.session_configured.session_id,
            child.session_configured.parent_thread_id
        ),
        (controller.identity(), Some(root_id))
    );
    assert_eq!(controller.service_tier(), Some("priority".to_string()));
    let message = maplecode_protocol::protocol::InterAgentCommunication::new(
        AgentPath::root(),
        AgentPath::root().join("worker").expect("valid child path"),
        Vec::new(),
        "Retain this in the host mailbox".into(),
        /*trigger_turn*/ false,
    );
    let unread_mail = vec![message.into()];
    *controller.mail.lock().expect("mail lock") = unread_mail.clone();
    let error = test
        .thread_manager
        .try_evict_v2_thread(Arc::clone(&child.thread))
        .await
        .expect_err("local eviction must reject a host-controlled child");
    assert!(matches!(
        error.details(),
        MapleCodeErrorDetails::InvalidRequest(_)
    ));
    assert!(Arc::ptr_eq(
        &test.thread_manager.get_thread(child.thread_id).await?,
        &child.thread,
    ));
    assert_eq!(controller.take_mailbox(child.thread_id), unread_mail);
    child.thread.ensure_rollout_materialized().await;
    child.thread.flush_rollout().await?;
    let saved = test
        .thread_store
        .load_latest_model_context(maplecode_thread_store::LoadThreadHistoryParams {
            thread_id: child.thread_id,
            include_archived: false,
        })
        .await?;
    assert!(saved.items.iter().any(|item| matches!(item,
        RolloutItem::SessionMeta(meta) if meta.meta.parent_thread_id == Some(root_id)
    )));

    controller
        .agents
        .lock()
        .expect("agents lock")
        .insert("/root/worker".into(), child.thread_id);
    for (thread, author, recipient, destination) in [
        (&test.maplecode, "/root", "/root/worker", "new_channel_name"),
        (&child.thread, "/root/worker", "/root", "channel_name"),
    ] {
        let arguments = serde_json::json!({
            (destination): "design",
            "text": "A shared decision.",
            "agents_to_notify": [recipient],
        });
        let mock = responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse(vec![
                    responses::ev_function_call_with_namespace(
                        "post",
                        "collaboration",
                        "post",
                        &arguments.to_string(),
                    ),
                    responses::ev_completed("post"),
                ]),
                responses::sse(vec![responses::ev_completed("done")]),
            ],
        )
        .await;
        thread
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Post a decision.".into(),
                text_elements: Vec::new(),
            }]))
            .await?;
        wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        let post: PostMetadata = serde_json::from_str(
            &mock.requests()[1]
                .function_call_output_text("post")
                .expect("post result"),
        )?;
        assert_eq!(post.author.as_str(), author);
    }

    // Rejected starts must leave the live tree's settings alone, whether the
    // requested root already exists or the factory returns the wrong identity.
    for reserved_thread_id in [root_id, ThreadId::new()] {
        let rejected = test
            .thread_manager
            .start_thread(StartThreadOptions {
                reserved_thread_id: Some(reserved_thread_id),
                environments: Some(vec![test.executor_environment().request()]),
                ..StartThreadOptions::new(child_config.clone())
            })
            .await;
        let error = rejected
            .err()
            .expect("startup must reject the requested root");
        if reserved_thread_id != root_id {
            assert!(
                matches!(error.details(), MapleCodeErrorDetails::InvalidRequest(_)),
                "{error:?}"
            );
        }
        assert_eq!(controller.service_tier(), Some("priority".to_string()));
    }
    child.thread.shutdown_and_wait().await?;
    test.thread_manager.remove_thread(&child.thread_id).await;
    let error = test
        .thread_manager
        .ensure_multi_agent_v2_child_loaded(child.thread_id)
        .await
        .expect_err("child reload must reach the host controller");
    assert!(
        matches!(error.details(), MapleCodeErrorDetails::InvalidRequest(message)
            if message == &format!("host resume rejection for {}", child.thread_id)
        ),
        "{error:?}"
    );
    test.maplecode.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_factory_follows_thread_lifecycle() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let test = test_maplecode()
        .with_config(|config| {
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("enable multi-agent v2");
        })
        .with_thread_manager({
            let calls = Arc::clone(&calls);
            move |manager| {
                manager.with_agent_control_factory(move |thread_id| {
                    calls.lock().expect("factory calls lock").push(thread_id);
                    let control: Arc<dyn AgentControl> = Arc::new(TestAgentControl::new(thread_id));
                    async move { Ok(control) }
                })
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let root_id = test.session_configured.thread_id;
    let root_tree_id = test.maplecode.agent_tree_id();
    let manager = &test.thread_manager;
    let options = || StartThreadOptions {
        environments: Some(vec![test.executor_environment().request()]),
        ..StartThreadOptions::new(test.config.clone())
    };
    let internal = manager
        .spawn_internal_session(
            root_id,
            StartThreadOptions {
                session_source: Some(SessionSource::Internal(InternalSessionSource::Guardian)),
                ..options()
            },
        )
        .await?;
    assert_eq!(
        internal.session_configured.session_id,
        test.session_configured.session_id
    );
    assert_eq!(internal.thread.agent_tree_id(), root_tree_id);
    test.maplecode.ensure_rollout_materialized().await;
    test.maplecode.flush_rollout().await?;
    let saved = test
        .maplecode
        .load_history(/*include_archived*/ false)
        .await?;
    let history = InitialHistory::Resumed(ResumedHistory {
        history_revision: None,
        conversation_id: root_id,
        history: Arc::new(saved.items),
        rollout_path: test.maplecode.rollout_path(),
    });
    let fork = manager
        .fork_thread_from_history(ForkSnapshot::Interrupted, options(), history.clone())
        .await?;
    assert_eq!(
        fork.session_configured.session_id,
        SessionId::from(fork.thread_id)
    );
    assert_ne!(fork.thread.agent_tree_id(), root_tree_id);
    let warm = manager
        .start_thread(StartThreadOptions {
            initial_history: history.clone(),
            ..options()
        })
        .await?;
    assert!(Arc::ptr_eq(&warm.thread, &test.maplecode));
    assert_eq!(warm.thread.agent_tree_id(), root_tree_id);
    assert_eq!(
        *calls.lock().expect("factory calls lock"),
        vec![root_id, fork.thread_id]
    );
    internal.thread.shutdown_and_wait().await?;
    test.maplecode.shutdown_and_wait().await?;
    let cold = manager
        .start_thread(StartThreadOptions {
            initial_history: history,
            ..options()
        })
        .await?;
    assert!(!Arc::ptr_eq(&cold.thread, &test.maplecode));
    assert_ne!(cold.thread.agent_tree_id(), root_tree_id);
    assert_eq!(cold.thread_id, root_id);
    assert_eq!(
        *calls.lock().expect("factory calls lock"),
        vec![root_id, fork.thread_id, root_id]
    );
    cold.thread.shutdown_and_wait().await?;
    fork.thread.shutdown_and_wait().await?;
    Ok(())
}

#[test_case(ThreadHistoryMode::Legacy; "legacy")]
#[test_case(ThreadHistoryMode::Paginated; "paginated")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_hosted_child_persists_inherited_session_id(
    history_mode: ThreadHistoryMode,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let root = test_maplecode().build_with_auto_env(&server).await?;
    let root_session_id = root.session_configured.session_id;
    let manager = &root.thread_manager;
    let options = || StartThreadOptions {
        history_mode: Some(history_mode),
        session_source: Some(SessionSource::Custom("host".to_string())),
        environments: Some(vec![root.executor_environment().request()]),
        ..StartThreadOptions::new(root.config.clone())
    };
    let mut child_options = options();
    child_options
        .thread_extension_init
        .insert(InheritedSessionId(root_session_id));
    let child = manager.start_thread(child_options).await?;
    assert_ne!(SessionId::from(child.thread_id), root_session_id);
    assert_eq!(
        (
            child.session_configured.session_id,
            child.session_configured.parent_thread_id
        ),
        (root_session_id, None)
    );
    child.thread.ensure_rollout_materialized().await;
    child.thread.flush_rollout().await?;
    let history = root
        .thread_store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id: child.thread_id,
            include_archived: false,
        })
        .await?;
    let Some(RolloutItem::SessionMeta(meta)) = history.items.first() else {
        anyhow::bail!("expected child metadata as the first rollout item");
    };
    assert_eq!(
        (meta.meta.id, meta.meta.session_id, meta.meta.history_mode),
        (child.thread_id, root_session_id, history_mode)
    );
    child.thread.shutdown_and_wait().await?;
    let resumed = manager
        .start_thread(StartThreadOptions {
            initial_history: InitialHistory::Resumed(ResumedHistory {
                history_revision: history.revision,
                conversation_id: child.thread_id,
                history: Arc::new(history.items),
                rollout_path: child.thread.rollout_path(),
            }),
            ..options()
        })
        .await?;
    assert!(!Arc::ptr_eq(&resumed.thread, &child.thread));
    assert_eq!(
        (
            resumed.thread_id,
            resumed.session_configured.session_id,
            resumed.session_configured.parent_thread_id
        ),
        (child.thread_id, root_session_id, None)
    );
    resumed.thread.shutdown_and_wait().await?;
    root.maplecode.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hosted_child_reports_terminal_turn_outcomes() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    const ANSWER: &str = "The patch looks good.";
    const FAILURE: &str = "The request was rejected.";
    const FOLLOWUP: &str = "Try again with the smaller patch.";
    const ASSIGNING_TURN: &str = "assigner-turn";

    struct Reporter {
        outcomes: mpsc::UnboundedSender<AgentTurnOutcome>,
        gate: Mutex<Option<oneshot::Receiver<()>>>,
    }

    impl AgentTurnOutcomeReporter for Reporter {
        fn report(&self, outcome: AgentTurnOutcome) -> BoxFuture<'_, ()> {
            Box::pin(async move {
                let failed = matches!(outcome.status, AgentStatus::Errored(_));
                self.outcomes.send(outcome).expect("report receiver");
                if failed {
                    let gate = self.gate.lock().expect("report gate").take();
                    gate.expect("one failure report")
                        .await
                        .expect("release failure report");
                }
            })
        }
    }

    let server = responses::start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(Arc::new(ThreadIdle));
    let root = test_maplecode()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;
    let session_id = root.session_configured.session_id;
    let (release_report, report_gate) = oneshot::channel();
    let (outcomes, mut reports) = mpsc::unbounded_channel();
    let reporter = Arc::new(Reporter {
        outcomes,
        gate: Mutex::new(Some(report_gate)),
    });
    let source = SessionSource::Custom("host".to_string());
    let mut options = StartThreadOptions {
        session_source: Some(source.clone()),
        thread_source: Some(ThreadSource::Subagent),
        ..root.start_thread_options().await
    };
    options
        .thread_extension_init
        .insert(InheritedSessionId(session_id));
    options
        .thread_extension_init
        .insert(reporter as Arc<dyn AgentTurnOutcomeReporter>);
    let child = root.thread_manager.start_thread(options).await?;
    let thread = &child.thread;

    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("review"),
                responses::ev_assistant_message("answer", ANSWER),
                responses::ev_completed("review"),
            ]),
            responses::sse_failed("failed", "invalid_prompt", FAILURE),
            responses::sse_completed("silent-followup"),
        ],
    )
    .await;
    let mut expect_report = async |expected_status| {
        let turn_id = wait_for_event_match(thread, |event| match event {
            EventMsg::TurnComplete(event) => Some(event.turn_id.clone()),
            _ => None,
        })
        .await;
        let report = tokio::time::timeout(Duration::from_secs(10), reports.recv())
            .await
            .expect("timed out waiting for the terminal turn report")
            .expect("reporter dropped");
        assert_eq!(
            (report.thread_id, &report.turn_id, &report.source),
            (child.thread_id, &turn_id, &source),
        );
        assert_eq!(
            (&report.status, report.error_info.as_ref()),
            (&expected_status, None)
        );
        report
    };
    for (prompt, expected_status) in [
        (
            "Review the patch.",
            AgentStatus::Completed(Some(ANSWER.into())),
        ),
        ("Fail this request.", AgentStatus::Errored(FAILURE.into())),
    ] {
        thread
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }]))
            .await?;
        let report = expect_report(expected_status).await;
        if !matches!(report.status, AgentStatus::Errored(_)) {
            ThreadIdle::wait(thread).await;
        }
    }

    // Queue the next task while the previous turn is still reporting its outcome.
    let assigner = AgentPath::root().join("assigner").expect("assigner path");
    let mail = InterAgentCommunication::new(
        assigner.clone(),
        AgentPath::root().join("reviewer").expect("child path"),
        Vec::new(),
        FOLLOWUP.into(),
        /*trigger_turn*/ true,
    );
    let followup = TurnInputRequest::new(TurnInput::InterAgentCommunication(mail)).on_start(
        TurnStartOptions {
            parent_turn_id: Some(ASSIGNING_TURN.into()),
            ..Default::default()
        },
    );
    assert_eq!(
        thread.start_or_steer_turn(followup).await?,
        TurnInputSubmission::Accepted
    );
    release_report.send(()).expect("release failure report");

    let report = expect_report(AgentStatus::Completed(None)).await;
    ThreadIdle::wait(thread).await;
    assert_eq!(
        (
            report.parent_turn_id.as_deref(),
            report.initiating_agent_path
        ),
        (Some(ASSIGNING_TURN), Some(assigner)),
    );
    assert_eq!(
        reports.try_recv().err(),
        Some(mpsc::error::TryRecvError::Empty)
    );
    thread.shutdown_and_wait().await?;
    let requests = responses::received_responses_requests(&server).await;
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[2].body_json()["client_metadata"]["turn_id"],
        report.turn_id
    );
    let messages = requests[2].inputs_of_type("agent_message");
    assert_eq!(messages.len(), 1);
    assert!(
        messages[0]["content"][0]["text"]
            .as_str()
            .is_some_and(|text| text.ends_with(FOLLOWUP))
    );
    root.maplecode.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test]
async fn shutdown_waits_for_host_outcome_handoff() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    #[derive(Default)]
    struct Handoff {
        entered: Notify,
        release: Notify,
        events: Mutex<Vec<&'static str>>,
    }

    impl Handoff {
        async fn pause(&self) {
            self.entered.notify_one();
            self.release.notified().await;
        }
    }

    impl TurnLifecycleContributor for Handoff {
        fn on_turn_stop<'a>(&'a self, _input: TurnStopInput<'a>) -> ExtensionFuture<'a, ()> {
            Box::pin(self.pause())
        }
    }

    impl AgentTurnOutcomeReporter for Handoff {
        fn report(&self, _outcome: AgentTurnOutcome) -> BoxFuture<'_, ()> {
            Box::pin(async move {
                self.pause().await;
                self.events.lock().expect("handoff events").push("reported");
            })
        }
    }

    impl ThreadLifecycleContributor<Config> for Handoff {
        fn on_thread_stop<'a>(&'a self, _input: ThreadStopInput<'a>) -> ExtensionFuture<'a, ()> {
            Box::pin(async move { self.events.lock().expect("handoff events").push("stopped") })
        }
    }

    let handoff = Arc::new(Handoff::default());
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.turn_lifecycle_contributor(handoff.clone());
    extensions.thread_lifecycle_contributor(handoff.clone());
    let server = responses::start_mock_server().await;
    let root = test_maplecode()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;
    let mut options = root.start_thread_options().await;
    options
        .thread_extension_init
        .insert(handoff.clone() as Arc<dyn AgentTurnOutcomeReporter>);
    let child = root.thread_manager.start_thread(options).await?;
    let thread = &child.thread;

    responses::mount_sse_once(&server, responses::sse_completed("done")).await;
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Finish the task.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;

    // The finalizer has detached its task, but has not entered the reporter.
    let wait_for_gate =
        async || tokio::time::timeout(Duration::from_secs(10), handoff.entered.notified()).await;
    wait_for_gate().await?;
    thread.submit(Op::Shutdown).await?;
    let assert_pending_then_release = async || {
        assert!(
            tokio::time::timeout(Duration::from_millis(250), thread.wait_until_terminated())
                .await
                .is_err(),
            "shutdown completed before the local handoff"
        );
        handoff.release.notify_one();
    };
    assert_pending_then_release().await;
    // The reporter has now been called, but the host has not accepted the outcome yet.
    wait_for_gate().await?;
    assert_pending_then_release().await;
    tokio::time::timeout(Duration::from_secs(10), thread.wait_until_terminated()).await?;
    assert_eq!(
        *handoff.events.lock().expect("handoff events"),
        ["reported", "stopped"]
    );
    root.maplecode.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_tools_dispatch_to_the_host_controller() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let (test, _controller) = test_with_host_control(&server, &[]).await?;
    let mut events = vec![responses::ev_response_created("tools")];
    for (call, name, arguments) in [
        (
            "spawn",
            "spawn_agent",
            serde_json::json!({"task_name": "worker", "message": "hello", "fork_turns": "none"}),
        ),
        (
            "send",
            "send_message",
            serde_json::json!({"target": ThreadId::new().to_string(), "message": "hello"}),
        ),
        ("list", "list_agents", serde_json::json!({})),
    ] {
        events.push(responses::ev_function_call_with_namespace(
            call,
            "collaboration",
            name,
            &arguments.to_string(),
        ));
    }
    events.push(responses::ev_completed("tools"));
    let calls = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(events),
            responses::sse(vec![responses::ev_completed("done")]),
        ],
    )
    .await;
    test.submit_text_turn("use collaboration").await?;
    let requests = calls.requests();
    assert_eq!(requests.len(), 2);
    for call in ["spawn", "send", "list"] {
        assert!(
            requests[1]
                .function_call_output(call)
                .to_string()
                .contains(&format!("host {call} rejection"))
        );
    }
    test.maplecode.shutdown_and_wait().await?;
    Ok(())
}
use maplecode_protocol::protocol::EventMsg;
use maplecode_protocol::protocol::InternalSessionSource;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_mailbox_notifies_waiting_agent_and_delivers_once() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let (test, controller) = test_with_host_control(&server, &[]).await?;
    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("waiting"),
            responses::ev_function_call_with_namespace(
                "wait-mail",
                "collaboration",
                "wait_agent",
                r#"{"timeout_ms":10000}"#,
            ),
            responses::ev_completed("waiting"),
        ]),
    )
    .await;
    test.maplecode
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Wait for a colleague.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.maplecode, |event| matches!(event,
        EventMsg::ItemStarted(event) if matches!(&event.item,
            maplecode_protocol::items::TurnItem::CollabAgentToolCall(call) if call.id == "wait-mail")
    )).await;
    let next = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("done"),
            responses::ev_assistant_message("done", "Received the note."),
            responses::ev_completed("done"),
        ]),
    )
    .await;
    controller.mail.lock().expect("mail lock").push(
        maplecode_protocol::protocol::InterAgentCommunication::new(
            AgentPath::root().join("worker").expect("valid worker path"),
            AgentPath::root(),
            Vec::new(),
            "Note from the host mailbox".into(),
            /*trigger_turn*/ false,
        )
        .into(),
    );
    controller.mailbox_state.send_replace(true);
    wait_for_event(&test.maplecode, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let request = next.single_request();
    let wait_output = request
        .function_call_output_text("wait-mail")
        .expect("wait_agent output");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&wait_output)?,
        serde_json::json!({"message": "Wait completed.", "timed_out": false})
    );
    assert_eq!(
        request
            .inputs_of_type("agent_message")
            .iter()
            .filter(|item| item.to_string().contains("Note from the host mailbox"))
            .count(),
        1
    );
    assert!(controller.mail.lock().expect("mail lock").is_empty());
    test.maplecode.shutdown_and_wait().await?;
    Ok(())
}

/// Both delivery paths wake an opted-in completed root exactly once.
#[test_case(false; "direct_op")]
#[test_case(true; "host_mailbox")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_mail_wakes_completed_root_when_enabled(
    via_host_mailbox: bool,
) -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let (test, controller) =
        test_with_host_control(&server, &[Feature::WakeOnAgentMessages]).await?;
    let mut final_answer = responses::ev_assistant_message("first-answer", "initial work finished");
    final_answer["item"]["phase"] = json!("final_answer");
    let response = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("initial"),
                final_answer,
                responses::ev_completed("initial"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("agent-mail"),
                responses::ev_assistant_message("mail-answer", "received the agent update"),
                responses::ev_completed("agent-mail"),
            ]),
        ],
    )
    .await;
    test.submit_text_turn("Finish the initial work.").await?;
    assert_eq!(response.requests().len(), 1);

    let message = InterAgentCommunication::new(
        AgentPath::root().join("worker").expect("valid worker path"),
        AgentPath::root(),
        Vec::new(),
        "Late agent update".into(),
        /*trigger_turn*/ false,
    );
    if via_host_mailbox {
        controller
            .mail
            .lock()
            .expect("mail lock")
            .push(message.into());
        controller.mailbox_state.send_replace(true);
    } else {
        test.maplecode
            .submit(Op::InterAgentCommunication {
                communication: message,
                start_options: Default::default(),
            })
            .await?;
    }
    wait_for_event(&test.maplecode, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.maplecode.shutdown_and_wait().await?;

    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        responses::received_responses_requests(&server).await.len(),
        2
    );
    assert_ne!(
        requests[0].body_json()["client_metadata"]["turn_id"],
        requests[1].body_json()["client_metadata"]["turn_id"]
    );
    let messages = requests[1].inputs_of_type("agent_message");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["author"], "/root/worker");
    assert_eq!(messages[0]["recipient"], "/root");
    assert_eq!(
        messages[0]["content"],
        json!([{"type": "input_text", "text": "Late agent update"}])
    );
    assert!(controller.mail.lock().expect("mail lock").is_empty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_start_requires_turn_admission() -> anyhow::Result<()> {
    struct HostWithoutAdmission(Mutex<Vec<&'static str>>);

    impl ThreadLifecycleContributor<Config> for HostWithoutAdmission {
        fn on_thread_start<'a>(
            &'a self,
            input: ThreadStartInput<'a, Config>,
        ) -> ExtensionFuture<'a, ()> {
            Box::pin(async move {
                self.0.lock().unwrap().push("start");
                input
                    .thread_store
                    .insert(maplecode_core::AgentRuntimeOwnership::Host);
            })
        }

        fn on_thread_stop<'a>(&'a self, _input: ThreadStopInput<'a>) -> ExtensionFuture<'a, ()> {
            Box::pin(async move { self.0.lock().unwrap().push("stop") })
        }
    }

    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let host = Arc::new(HostWithoutAdmission(Mutex::default()));
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(host.clone());
    let error = test_maplecode()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await
        .err()
        .expect("missing admission")
        .downcast::<MapleCodeErr>()?;
    assert!(
        matches!(error.details(), MapleCodeErrorDetails::InvalidRequest(message) if message.contains("AgentTurnAdmission")),
        "{error:?}"
    );
    assert_eq!(*host.0.lock().unwrap(), ["start", "stop"]);
    Ok(())
}
