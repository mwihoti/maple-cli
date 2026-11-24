#![allow(clippy::expect_used)]

use std::sync::Arc;
use std::sync::Mutex;

use maplecode_extension_api::ApprovalReviewContributor;
use maplecode_extension_api::ConfigContributor;
use maplecode_extension_api::ContentItemKind;
use maplecode_extension_api::ContextContributor;
use maplecode_extension_api::ContextualUserFragment;
use maplecode_extension_api::ConversationHistorySnapshot;
use maplecode_extension_api::ExtensionData;
use maplecode_extension_api::ExtensionDataInit;
use maplecode_extension_api::ExtensionEventSink;
use maplecode_extension_api::ExtensionFuture;
use maplecode_extension_api::ExtensionMetrics;
use maplecode_extension_api::ExtensionRegistryBuilder;
use maplecode_extension_api::ExtensionWarning;
use maplecode_extension_api::McpServerContributionContext;
use maplecode_extension_api::PromptFragment;
use maplecode_extension_api::ResponseItem;
use maplecode_extension_api::SkillInvocationContributor;
use maplecode_extension_api::ThreadLifecycleContributor;
use maplecode_extension_api::TokenUsageContributor;
use maplecode_extension_api::ToolCall;
use maplecode_extension_api::ToolContributor;
use maplecode_extension_api::ToolExecutor;
use maplecode_extension_api::ToolLifecycleContributor;
use maplecode_extension_api::TurnContextContributionInput;
use maplecode_extension_api::TurnInputContext;
use maplecode_extension_api::TurnInputContributor;
use maplecode_extension_api::TurnItemContributor;
use maplecode_extension_api::TurnLifecycleContributor;
use maplecode_protocol::items::HookPromptItem;
use maplecode_protocol::items::TurnItem;
use maplecode_protocol::protocol::EnvironmentConfigState;
use maplecode_protocol::protocol::Event;
use maplecode_protocol::protocol::EventMsg;
use maplecode_protocol::protocol::SessionSource;
use maplecode_protocol::protocol::SubAgentSource;
use maplecode_protocol::protocol::TurnEnvironmentRequest;
use maplecode_protocol::protocol::TurnEnvironmentSelection;
use maplecode_protocol::protocol::WarningEvent;
use maplecode_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

struct AllContributors;

#[test]
fn mcp_contribution_context_identifies_the_running_thread() {
    let config = ();
    let thread_init = ExtensionDataInit::new();
    let thread_store = ExtensionData::new("child-thread");
    let session_source = SessionSource::SubAgent(SubAgentSource::Review);

    let thread_context = McpServerContributionContext::for_step(
        &config,
        &thread_init,
        &thread_store,
        "maplecode_work_cca",
        &[],
        /*executor_capability_discovery*/ None,
        /*executor_capability_discovery_v2*/ None,
    )
    .with_session_source(&session_source);

    assert_eq!(thread_context.session_source(), Some(&session_source));
    assert_eq!(
        McpServerContributionContext::global(&config).session_source(),
        None
    );
}

#[test]
fn mcp_context_preserves_selected_executor_order_and_unavailability() {
    let config = ();
    let environments = vec![
        TurnEnvironmentSelection::new(
            TurnEnvironmentRequest {
                environment_id: "starting-primary".to_owned(),
                cwd: PathUri::parse("file:///workspace").expect("workspace URI"),
                workspace_roots: Vec::new(),
                config: EnvironmentConfigState::Pending,
            },
            &[],
        ),
        TurnEnvironmentSelection::new(
            TurnEnvironmentRequest {
                environment_id: "ready-secondary".to_owned(),
                cwd: PathUri::parse("file:///other").expect("workspace URI"),
                workspace_roots: Vec::new(),
                config: EnvironmentConfigState::FromThread,
            },
            &[],
        ),
    ];
    let context = McpServerContributionContext::global(&config);
    assert_eq!(context.selected_environments(), None);
    assert_eq!(
        context
            .with_selected_environments(&[])
            .selected_environments(),
        Some([].as_slice())
    );
    assert_eq!(
        context
            .with_selected_environments(&environments)
            .selected_environments(),
        Some(environments.as_slice())
    );
}

impl ContextContributor for AllContributors {
    fn contribute_thread_context<'a>(
        &'a self,
        _session_store: &'a ExtensionData,
        _thread_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<PromptFragment>> {
        Box::pin(std::future::ready(Vec::new()))
    }
}

impl ThreadLifecycleContributor<()> for AllContributors {}

impl TurnLifecycleContributor for AllContributors {}

impl ConfigContributor<()> for AllContributors {}

impl TokenUsageContributor for AllContributors {}

impl SkillInvocationContributor for AllContributors {}

struct ExecutorOnlySkillContributor;

impl SkillInvocationContributor for ExecutorOnlySkillContributor {
    fn requires_host_skill_discovery(&self) -> bool {
        false
    }
}

#[test]
fn host_skill_discovery_preserves_legacy_and_host_contributor_behavior() {
    assert!(
        ExtensionRegistryBuilder::<()>::new()
            .build()
            .requires_host_skill_discovery()
    );

    let mut executor_only = ExtensionRegistryBuilder::<()>::new();
    executor_only.skill_invocation_contributor(Arc::new(ExecutorOnlySkillContributor));
    assert!(!executor_only.build().requires_host_skill_discovery());

    let mut mixed = ExtensionRegistryBuilder::<()>::new();
    mixed.skill_invocation_contributor(Arc::new(ExecutorOnlySkillContributor));
    mixed.skill_invocation_contributor(Arc::new(AllContributors));
    assert!(mixed.build().requires_host_skill_discovery());
}

impl TurnInputContributor for AllContributors {
    fn contribute<'a>(
        &'a self,
        input: TurnInputContext<'a>,
        _extension_metrics: Option<Arc<dyn ExtensionMetrics>>,
        _session_store: &'a ExtensionData,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<Box<dyn ContextualUserFragment + Send>>> {
        Box::pin(async move {
            let _self = self;
            let _input = input;
            Vec::new()
        })
    }
}

impl ToolContributor for AllContributors {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        Vec::new()
    }
}

impl ToolLifecycleContributor for AllContributors {}

impl TurnItemContributor for AllContributors {
    fn contribute<'a>(
        &'a self,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
        _item: &'a mut TurnItem,
    ) -> ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            let _self = self;
            Ok(())
        })
    }
}

impl ApprovalReviewContributor for AllContributors {
    fn decide<'a>(
        &'a self,
        _input: &'a maplecode_extension_api::ApprovalDecisionInput<'_>,
    ) -> ExtensionFuture<'a, Option<maplecode_extension_api::ApprovalDecision>> {
        Box::pin(async { Some(maplecode_extension_api::ApprovalDecision::AskUser) })
    }
}

impl maplecode_extension_api::SynchronousApprovalReviewer for AllContributors {
    fn review<'a>(
        &'a self,
        _reason: maplecode_protocol::approvals::GuardianReviewReason,
        _async_approval: Option<maplecode_extension_api::ExtensionFuture<'a, ()>>,
    ) -> ExtensionFuture<'a, Option<maplecode_protocol::protocol::ReviewDecision>> {
        Box::pin(std::future::ready(Some(
            maplecode_protocol::protocol::ReviewDecision::Approved,
        )))
    }
}

#[tokio::test]
async fn build_round_trips_every_contributor_category() {
    let contributor = Arc::new(AllContributors);
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    builder.thread_lifecycle_contributor(contributor.clone());
    builder.turn_lifecycle_contributor(contributor.clone());
    builder.config_contributor(contributor.clone());
    builder.token_usage_contributor(contributor.clone());
    builder.skill_invocation_contributor(contributor.clone());
    builder.prompt_contributor(contributor.clone());
    builder.turn_input_contributor(contributor.clone());
    builder.tool_contributor(contributor.clone());
    builder.tool_lifecycle_contributor(contributor.clone());
    builder.turn_item_contributor(contributor.clone());
    builder.approval_review_contributor(contributor);
    let registry = builder.build();

    assert_eq!(registry.thread_lifecycle_contributors().len(), 1);
    assert_eq!(registry.turn_lifecycle_contributors().len(), 1);
    assert_eq!(registry.config_contributors().len(), 1);
    assert_eq!(registry.token_usage_contributors().len(), 1);
    assert_eq!(registry.skill_invocation_contributors().len(), 1);
    assert_eq!(registry.context_contributors().len(), 1);
    assert_eq!(registry.turn_input_contributors().len(), 1);
    assert_eq!(registry.tool_contributors().len(), 1);
    assert_eq!(registry.tool_lifecycle_contributors().len(), 1);
    assert_eq!(registry.turn_item_contributors().len(), 1);
    let thread_store = ExtensionData::new("thread");
    let input = maplecode_extension_api::ApprovalDecisionInput {
        serving_assessment: None,
        permissions: Some(&Default::default()),
        approval_id: "approval-1",
        tool_call_id: None,
        action: &serde_json::Value::Null,
        thread_id: maplecode_protocol::ThreadId::new(),
        thread_store: &thread_store,
        turn_store: &thread_store,
        category: maplecode_protocol::openai_models::GuardianScope::Shell,
        approval_policy: maplecode_protocol::protocol::AskForApproval::OnRequest,
        approvals_reviewer: maplecode_protocol::config_types::ApprovalsReviewer::AutoReview,
        require_guardian: false,
        require_fresh_review: false,
        full_access: false,
        metrics: None,
        synchronous_reviewer: &AllContributors,
    };
    assert_eq!(
        registry.decide_approval(&input).await,
        Some(maplecode_extension_api::ApprovalDecision::AskUser)
    );
}

impl ConversationHistorySnapshot for AllContributors {
    fn history_version(&self) -> u64 {
        0
    }

    fn user_message_revision(&self) -> u64 {
        0
    }

    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(std::iter::empty())
    }
}

struct NamedContextContributor(&'static str);

impl ContextContributor for NamedContextContributor {
    fn contribute_thread_context<'a>(
        &'a self,
        _session_store: &'a ExtensionData,
        _thread_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<PromptFragment>> {
        Box::pin(std::future::ready(vec![PromptFragment::developer_policy(
            self.0,
            ContentItemKind("test.thread_context".to_string()),
        )]))
    }
}

struct NamedTurnContextContributor(&'static str);

impl ContextContributor for NamedTurnContextContributor {
    fn contribute_turn_context<'a>(
        &'a self,
        _input: TurnContextContributionInput<'a>,
    ) -> ExtensionFuture<'a, Vec<PromptFragment>> {
        Box::pin(std::future::ready(vec![
            PromptFragment::developer_capability(
                self.0,
                ContentItemKind("test.turn_context".to_string()),
            ),
        ]))
    }
}

struct RecordingTurnItemContributor {
    name: &'static str,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl TurnItemContributor for RecordingTurnItemContributor {
    fn contribute<'a>(
        &'a self,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
        _item: &'a mut TurnItem,
    ) -> ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            self.calls
                .lock()
                .expect("turn item calls lock should not be poisoned")
                .push(self.name);
            Ok(())
        })
    }
}

#[tokio::test]
async fn contributors_preserve_registration_order() {
    let turn_item_calls = Arc::new(Mutex::new(Vec::new()));
    let mut builder = ExtensionRegistryBuilder::<()>::new();
    builder.prompt_contributor(Arc::new(NamedContextContributor("first")));
    builder.prompt_contributor(Arc::new(NamedContextContributor("second")));
    builder.prompt_contributor(Arc::new(NamedTurnContextContributor("turn-first")));
    builder.prompt_contributor(Arc::new(NamedTurnContextContributor("turn-second")));
    for name in ["first", "second"] {
        builder.turn_item_contributor(Arc::new(RecordingTurnItemContributor {
            name,
            calls: Arc::clone(&turn_item_calls),
        }));
    }
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let turn_store = ExtensionData::new("turn");

    let mut fragments = Vec::new();
    for contributor in registry.context_contributors() {
        fragments.extend(
            contributor
                .contribute_thread_context(&session_store, &thread_store)
                .await,
        );
    }
    for contributor in registry.context_contributors() {
        fragments.extend(
            contributor
                .contribute_turn_context(TurnContextContributionInput {
                    thread_id: maplecode_protocol::ThreadId::default(),
                    turn_id: turn_store.level_id(),
                    session_store: &session_store,
                    thread_store: &thread_store,
                    turn_store: &turn_store,
                    model_context_window: Some(123),
                })
                .await,
        );
    }
    let mut item = TurnItem::HookPrompt(HookPromptItem {
        id: "item".to_string(),
        fragments: Vec::new(),
    });
    for contributor in registry.turn_item_contributors() {
        contributor
            .contribute(&thread_store, &turn_store, &mut item)
            .await
            .expect("turn item contribution should succeed");
    }

    assert_eq!(
        fragments,
        vec![
            PromptFragment::developer_policy(
                "first",
                ContentItemKind("test.thread_context".to_string()),
            ),
            PromptFragment::developer_policy(
                "second",
                ContentItemKind("test.thread_context".to_string()),
            ),
            PromptFragment::developer_capability(
                "turn-first",
                ContentItemKind("test.turn_context".to_string()),
            ),
            PromptFragment::developer_capability(
                "turn-second",
                ContentItemKind("test.turn_context".to_string()),
            ),
        ]
    );
    assert_eq!(
        turn_item_calls
            .lock()
            .expect("turn item calls lock")
            .as_slice(),
        ["first", "second"]
    );
}

#[derive(Default)]
struct RecordingEventSink {
    events: Mutex<Vec<(String, String)>>,
}

impl ExtensionEventSink for RecordingEventSink {
    fn emit(&self, event: Event) {
        let EventMsg::Warning(warning) = event.msg else {
            panic!("test sink only accepts warning events");
        };
        self.events
            .lock()
            .expect("recording event sink lock should not be poisoned")
            .push((event.id, warning.message));
    }

    fn emit_warning(&self, warning: ExtensionWarning) {
        self.events
            .lock()
            .expect("recording event sink lock should not be poisoned")
            .push((warning.thread_id, warning.message));
    }
}

#[test]
fn custom_event_sink_survives_registry_build() {
    let sink = Arc::new(RecordingEventSink::default());
    let builder = ExtensionRegistryBuilder::<()>::with_event_sink(sink.clone());

    builder
        .event_sink()
        .emit(warning_event("builder", "before"));
    let registry = builder.build();
    registry
        .event_sink()
        .emit(warning_event("registry", "after"));
    registry.event_sink().emit_warning(ExtensionWarning {
        thread_id: "thread".to_string(),
        turn_id: Some("turn".to_string()),
        message: "warning".to_string(),
    });

    assert_eq!(
        sink.events
            .lock()
            .expect("recording event sink lock")
            .as_slice(),
        [
            ("builder".to_string(), "before".to_string()),
            ("registry".to_string(), "after".to_string()),
            ("thread".to_string(), "warning".to_string()),
        ]
    );
}

fn warning_event(id: &str, message: &str) -> Event {
    Event {
        id: id.to_string(),
        msg: EventMsg::Warning(WarningEvent {
            message: message.to_string(),
        }),
    }
}
