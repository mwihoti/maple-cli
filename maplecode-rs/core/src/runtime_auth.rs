//! Construct and share the harness-owned runtime credential context.

use crate::config::Config;
use maplecode_features::Feature;
use maplecode_login::AuthManager;
use maplecode_protocol::error::Result as MapleCodeResult;
use maplecode_runtime_agent_instance::RuntimeAuthManager;
use std::sync::Arc;
use std::sync::OnceLock;

// A later thread may enable runtime auth even when harness startup left it disabled.
pub(crate) type RuntimeAuthSlot = Arc<OnceLock<Arc<RuntimeAuthManager>>>;

/// Share this owner between thread sessions and first-party API clients.
/// Construction does not register a runtime or generate its key.
pub fn build_runtime_auth_manager(
    config: &Config,
    credentials: Arc<AuthManager>,
    installation_id: &str,
) -> Option<Arc<RuntimeAuthManager>> {
    runtime_for_config(
        &RuntimeAuthSlot::default(),
        config,
        credentials,
        installation_id,
    )
}

pub(crate) fn runtime_for_config(
    slot: &RuntimeAuthSlot,
    config: &Config,
    credentials: Arc<AuthManager>,
    installation_id: &str,
) -> Option<Arc<RuntimeAuthManager>> {
    config
        .features
        .enabled(Feature::RuntimeAgentInstances)
        .then(|| {
            Arc::clone(slot.get_or_init(|| {
                Arc::new(RuntimeAuthManager::for_maplecode(
                    credentials,
                    &config.chatgpt_base_url,
                    installation_id,
                ))
            }))
        })
}

pub(crate) fn is_openai_realtime_url(base_url: &str) -> MapleCodeResult<bool> {
    let Ok(mut url) = url::Url::parse(base_url) else {
        return Ok(false);
    };
    if url.scheme() == "wss" && url.set_scheme("https").is_err() {
        return Ok(false);
    }
    maplecode_model_provider::validate_runtime_destination(Some(url.as_str()))?;
    Ok(maplecode_runtime_agent_instance::is_openai_api_url(
        url.as_str(),
    ))
}

#[cfg(test)]
#[path = "runtime_auth_tests.rs"]
mod tests;
