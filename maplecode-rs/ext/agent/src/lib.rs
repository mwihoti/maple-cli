use maplecode_core::MaplecodeThread;
use maplecode_core::NewThread;
use maplecode_core::StartIfIdleSubmission;
use maplecode_core::StartThreadOptions;
use maplecode_core::ThreadManager;
use maplecode_core::TurnInputRequest;
use maplecode_core::config::Config;
use maplecode_protocol::ThreadId;
use maplecode_protocol::error::MaplecodeErr;
use maplecode_protocol::error::Result as MaplecodeResult;
use maplecode_protocol::protocol::W3cTraceContext;
use maplecode_protocol::user_input::UserInput;
use std::sync::Arc;
use std::sync::Weak;

/// A fully resolved agent invocation.
///
/// Agent discovery owns rendering `prompt`, including any selected skill
/// references. The runtime only starts that prompt in isolated forked context.
pub struct AgentInvocation {
    pub config: Config,
    pub prompt: String,
    pub parent_trace: Option<W3cTraceContext>,
}

/// A spawned agent whose initial turn has been submitted.
pub struct AgentRun {
    pub thread_id: ThreadId,
    pub turn_id: String,
    pub thread: Arc<MaplecodeThread>,
}

/// Runs resolved agents in legacy threads forked by the owning [`ThreadManager`].
#[derive(Clone)]
pub struct AgentRunner {
    thread_manager: Weak<ThreadManager>,
}

impl AgentRunner {
    pub fn new(thread_manager: Weak<ThreadManager>) -> Self {
        Self { thread_manager }
    }

    /// Starts a resolved agent in a fork of a legacy `parent_thread_id`.
    ///
    /// Paginated parents are rejected by [`ThreadManager::spawn_legacy_subagent`].
    pub async fn start(
        &self,
        parent_thread_id: ThreadId,
        invocation: AgentInvocation,
    ) -> MaplecodeResult<AgentRun> {
        let AgentInvocation {
            config,
            prompt,
            parent_trace,
        } = invocation;
        if prompt.trim().is_empty() {
            return Err(MaplecodeErr::InvalidRequest(
                "agent prompt must not be empty".to_string(),
            ));
        }

        let thread_manager = self.thread_manager.upgrade().ok_or_else(|| {
            MaplecodeErr::UnsupportedOperation("thread manager dropped".to_string())
        })?;
        let NewThread {
            thread_id, thread, ..
        } = thread_manager
            .spawn_legacy_subagent(
                parent_thread_id,
                StartThreadOptions {
                    parent_trace: parent_trace.clone(),
                    ..StartThreadOptions::new(config)
                },
            )
            .await?;
        let turn_id = match thread
            .start_turn_if_idle(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }])
                .with_trace(parent_trace),
            )
            .await?
        {
            StartIfIdleSubmission::Started { turn_id, .. } => turn_id,
            StartIfIdleSubmission::NotSubmitted { reason } => {
                return Err(MaplecodeErr::InvalidRequest(format!(
                    "agent prompt was not submitted: {reason:?}"
                )));
            }
        };

        Ok(AgentRun {
            thread_id,
            turn_id,
            thread,
        })
    }
}
