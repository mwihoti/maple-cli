//! Applies V2 interruption rules to a registered agent without loading its runtime.
//!
//! Root and self targets are rejected. An unloaded or already-dead runtime is a successful
//! interruption; this operation never reloads it.

use super::LocalAgentControl;
use crate::agent::api::AgentInfo;
use maplecode_protocol::AgentPath;
use maplecode_protocol::ThreadId;
use maplecode_protocol::error::MapleCodeErr;
use maplecode_protocol::error::MapleCodeErrorDetails;
use maplecode_protocol::error::Result as MapleCodeResult;

impl LocalAgentControl {
    /// Interrupts a spawned agent's current task, preserving the status observed before dispatch.
    pub(crate) async fn interrupt_spawned_agent(
        &self,
        caller: ThreadId,
        target: ThreadId,
    ) -> MapleCodeResult<AgentInfo> {
        let receiver_agent = self.runtime.ensure_agent_known(target)?;
        if receiver_agent
            .agent_path
            .as_ref()
            .is_some_and(AgentPath::is_root)
        {
            return Err(MapleCodeErr::UnsupportedOperation(
                "root is not a spawned agent".to_string(),
            ));
        }
        if target == caller {
            return Err(MapleCodeErr::UnsupportedOperation(
                "an agent cannot interrupt itself; return your result and let the parent interrupt you if needed"
                    .to_string(),
            ));
        }
        receiver_agent.agent_path.as_ref().ok_or_else(|| {
            MapleCodeErr::UnsupportedOperation("target agent is missing an agent_path".to_string())
        })?;
        let snapshot = self.inspect_agent(target).await?;
        match self.interrupt_agent(target).await {
            Ok(_) => {}
            Err(err)
                if matches!(
                    err.details(),
                    MapleCodeErrorDetails::ThreadNotFound(_)
                        | MapleCodeErrorDetails::InternalAgentDied
                ) => {}
            Err(err) => return Err(err),
        }
        Ok(snapshot)
    }
}
