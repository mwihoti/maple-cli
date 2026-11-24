use super::*;
use crate::create_model_provider;
use maplecode_model_provider_info::ModelProviderCapabilities;
use pretty_assertions::assert_eq;

#[test]
fn capability_overrides_preserve_unspecified_provider_defaults() {
    for info in [
        ModelProviderInfo::default(),
        ModelProviderInfo::create_openai_provider(/*base_url*/ None),
        ModelProviderInfo {
            name: "Azure".to_string(),
            ..Default::default()
        },
    ] {
        let defaults = create_model_provider(info.clone(), /*auth_manager*/ None).capabilities();
        let provider = create_model_provider(
            ModelProviderInfo {
                capabilities: Some(ModelProviderCapabilities {
                    external_web_access: Some(false),
                    ..Default::default()
                }),
                ..info
            },
            /*auth_manager*/ None,
        );
        assert_eq!(
            provider.capabilities(),
            ProviderCapabilities {
                external_web_access: false,
                ..defaults
            },
        );
    }
}

#[test]
fn maple_provider_disables_hosted_tools() {
    let capabilities = create_model_provider(
        maplecode_model_provider_info::create_maple_provider(),
        /*auth_manager*/ None,
    )
    .capabilities();
    assert!(!capabilities.web_search);
    assert!(!capabilities.image_generation);
    assert!(!capabilities.external_web_access);
}
