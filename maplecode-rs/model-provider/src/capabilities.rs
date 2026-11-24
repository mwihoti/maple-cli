//! Provider capability defaults and resolution of custom provider overrides.

use maplecode_api::is_azure_responses_provider;
use maplecode_model_provider_info::MAPLE_PROVIDER_NAME;
use maplecode_model_provider_info::ModelProviderInfo;
use maplecode_model_provider_info::RemoteCompactionSupport;

/// Optional provider-backed features that Maple Code may expose at runtime.
///
/// These capabilities are a provider-owned upper bound. Model and turn settings
/// can further restrict the functionality exposed to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    /// Whether requests may include the Responses API `reasoning.summary` parameter.
    pub reasoning_summaries: bool,
    pub image_generation: bool,
    pub web_search: bool,
    pub external_web_access: bool,
    pub remote_compaction: RemoteCompactionSupport,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            reasoning_summaries: true,
            image_generation: true,
            web_search: true,
            external_web_access: true,
            remote_compaction: RemoteCompactionSupport::Unsupported,
        }
    }
}

impl ProviderCapabilities {
    pub(crate) fn from_config(info: &ModelProviderInfo) -> Self {
        let defaults = Self {
            remote_compaction: if info.is_openai()
                || is_azure_responses_provider(&info.name, info.base_url.as_deref())
            {
                RemoteCompactionSupport::V2
            } else {
                RemoteCompactionSupport::Unsupported
            },
            ..Self::default()
        };
        // Maple translates requests to Chat Completions and only accepts direct
        // local function/custom tools; hosted tools would fail every turn.
        let hosted_tools = info.name != MAPLE_PROVIDER_NAME;
        let overrides = info.capabilities.unwrap_or_default();
        Self {
            web_search: hosted_tools && defaults.web_search,
            image_generation: hosted_tools && defaults.image_generation,
            external_web_access: overrides
                .external_web_access
                .unwrap_or(defaults.external_web_access),
            remote_compaction: overrides
                .remote_compaction
                .unwrap_or(defaults.remote_compaction),
            ..defaults
        }
    }
}

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
