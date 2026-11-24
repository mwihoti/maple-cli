use anyhow::Context;
use std::path::Path;

use maplecode_app_server_protocol::ServerNotification;
use maplecode_core::config::Config;
use maplecode_exec_server::BoundOutput;
use maplecode_protocol::protocol::SessionConfiguredEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapleCodeStatus {
    Running,
    InitiateShutdown,
}

pub(crate) trait EventProcessor {
    /// Print summary of effective configuration and user prompt.
    fn print_config_summary(
        &mut self,
        config: &Config,
        prompt: &str,
        session_configured: &SessionConfiguredEvent,
    );

    /// Handle a single typed app-server notification emitted by the agent.
    fn process_server_notification(&mut self, notification: ServerNotification) -> MapleCodeStatus;

    /// Handle a local exec warning that is not represented as an app-server notification.
    fn process_warning(&mut self, message: String) -> MapleCodeStatus;

    fn print_final_output(&mut self, output: Option<BoundOutput>) -> anyhow::Result<()>;
}

pub(crate) fn handle_last_message(
    last_agent_message: Option<&str>,
    path: &Path,
    file: BoundOutput,
) -> anyhow::Result<()> {
    file.write(last_agent_message.unwrap_or_default().as_bytes())
        .with_context(|| format!("Failed to write last message file {path:?}"))?;
    if last_agent_message.is_none() {
        eprintln!(
            "Warning: no last agent message; wrote empty content to {}",
            path.display()
        );
    }
    Ok(())
}
