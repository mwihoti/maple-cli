use std::sync::Arc;

use maplecode_config::McpServerTransportConfig;
use maplecode_config::test_support::CloudConfigBundleFixture;
use maplecode_core::McpManager;
use maplecode_core::config::Config;
use maplecode_core::config::ConfigBuilder;
use maplecode_core::plugins_manager_for_config;
use maplecode_extension_api::ExtensionRegistryBuilder;
use maplecode_extension_api::McpServerContribution;
use maplecode_extension_api::McpServerContributionContext;
use maplecode_extension_api::McpServerContributor;
use maplecode_login::AuthManager;
use maplecode_login::MapleCodeAuth;
use maplecode_login::test_support::auth_manager_from_optional_auth;
use maplecode_mcp::MAPLECODE_APPS_MCP_SERVER_NAME;
use maplecode_mcp::McpServerKey;
use pretty_assertions::assert_eq;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn contributes_hosted_plugin_runtime_without_an_executor() -> TestResult {
    let maplecode_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .maplecode_home(maplecode_home.path().to_path_buf())
        .fallback_cwd(Some(maplecode_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            ("chatgpt_base_url".to_string(), "https://chatgpt.com".into()),
        ])
        .build()
        .await?;
    let auth = MapleCodeAuth::create_dummy_chatgpt_auth_for_testing();
    let manager = installed_manager(&config, Some(auth.clone()));

    let servers = manager.effective_servers(&config, Some(&auth)).await;
    let server = servers
        .get(&McpServerKey::unscoped(MAPLECODE_APPS_MCP_SERVER_NAME))
        .ok_or("hosted plugin runtime should be contributed as a configured server")?
        .config();
    let McpServerTransportConfig::StreamableHttp { url, .. } = &server.transport else {
        panic!("hosted plugin runtime should use streamable HTTP");
    };
    assert_eq!(url, "https://chatgpt.com/backend-api/ps/mcp");

    Ok(())
}

#[tokio::test]
async fn runtime_overlay_preserves_disabled_server() -> TestResult {
    let maplecode_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .maplecode_home(maplecode_home.path().to_path_buf())
        .fallback_cwd(Some(maplecode_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            (
                "mcp_servers.maplecode_apps.url".to_string(),
                "https://example.com/mcp".into(),
            ),
            (
                "mcp_servers.maplecode_apps.enabled".to_string(),
                false.into(),
            ),
        ])
        .cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement("[mcp_servers]"),
        )
        .build()
        .await?;
    let auth = MapleCodeAuth::create_dummy_chatgpt_auth_for_testing();
    let manager = installed_manager(&config, Some(auth.clone()));

    let configured = manager.configured_servers(&config).await;
    // Standalone login must still reject requirements-denied, explicitly disabled entries.
    let login_client = Arc::new(maplecode_exec_server::RouteAwareHttpClient::new(
        config.http_client_factory(),
    ));
    assert_eq!(
        maplecode_mcp::apply_http_headers_helper(
            login_client,
            &configured[MAPLECODE_APPS_MCP_SERVER_NAME],
            maplecode_home.path().to_path_buf(),
        )
        .err(),
        Some("the MCP server is disabled by managed requirements".to_string())
    );
    let servers = manager.effective_servers(&config, Some(&auth)).await;
    let server = servers
        .get(&McpServerKey::unscoped(MAPLECODE_APPS_MCP_SERVER_NAME))
        .ok_or("hosted plugin runtime should remain configured")?;

    assert!(!server.enabled());
    Ok(())
}

#[tokio::test]
async fn default_fallback_overwrites_reserved_config_without_an_extension() -> TestResult {
    let maplecode_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .maplecode_home(maplecode_home.path().to_path_buf())
        .fallback_cwd(Some(maplecode_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), true.into()),
            (
                "mcp_servers.maplecode_apps.url".to_string(),
                "https://example.com/mcp".into(),
            ),
        ])
        .build()
        .await?;
    let auth = MapleCodeAuth::create_dummy_chatgpt_auth_for_testing();
    let manager = McpManager::new(Arc::new(plugins_manager_for_config(
        &config,
        AuthManager::from_auth_for_testing(auth.clone()),
    )));

    let servers = manager.effective_servers(&config, Some(&auth)).await;
    let server = servers
        .get(&McpServerKey::unscoped(MAPLECODE_APPS_MCP_SERVER_NAME))
        .ok_or("default Apps MCP should be present")?
        .config();
    let McpServerTransportConfig::StreamableHttp { url, .. } = &server.transport else {
        panic!("default Apps MCP should use streamable HTTP");
    };
    assert_eq!(url, "https://chatgpt.com/backend-api/ps/mcp");

    Ok(())
}

#[tokio::test]
async fn later_extension_can_remove_same_name_registration() -> TestResult {
    let maplecode_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .maplecode_home(maplecode_home.path().to_path_buf())
        .fallback_cwd(Some(maplecode_home.path().to_path_buf()))
        .cli_overrides(vec![("features.apps".to_string(), true.into())])
        .build()
        .await?;
    let auth = MapleCodeAuth::create_dummy_chatgpt_auth_for_testing();
    let mut builder = ExtensionRegistryBuilder::new();
    maplecode_mcp_extension::install(&mut builder);
    builder.mcp_server_contributor(Arc::new(RemoveMapleCodeApps));
    let manager = McpManager::new_with_extensions(
        Arc::new(plugins_manager_for_config(
            &config,
            AuthManager::from_auth_for_testing(auth.clone()),
        )),
        Arc::new(builder.build()),
        maplecode_core::MapleCodeAppsToolsCache::default(),
    );

    let servers = manager.effective_servers(&config, Some(&auth)).await;

    assert!(!servers.contains_key(&McpServerKey::unscoped(MAPLECODE_APPS_MCP_SERVER_NAME)));
    Ok(())
}

#[tokio::test]
async fn hosted_apps_mcp_requires_chatgpt_auth() -> TestResult {
    let maplecode_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .maplecode_home(maplecode_home.path().to_path_buf())
        .fallback_cwd(Some(maplecode_home.path().to_path_buf()))
        .cli_overrides(vec![("features.apps".to_string(), true.into())])
        .build()
        .await?;
    let auth = MapleCodeAuth::from_api_key("test");
    let manager = installed_manager(&config, Some(auth.clone()));

    let servers = manager.effective_servers(&config, Some(&auth)).await;
    assert!(!servers.contains_key(&McpServerKey::unscoped(MAPLECODE_APPS_MCP_SERVER_NAME)));

    Ok(())
}

#[tokio::test]
async fn disabled_apps_remove_reserved_server_config_for_all_hosts() -> TestResult {
    let maplecode_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .maplecode_home(maplecode_home.path().to_path_buf())
        .fallback_cwd(Some(maplecode_home.path().to_path_buf()))
        .cli_overrides(vec![
            ("features.apps".to_string(), false.into()),
            (
                "mcp_servers.maplecode_apps.url".to_string(),
                "https://example.com/mcp".into(),
            ),
        ])
        .build()
        .await?;
    let managers = [
        installed_manager(&config, /*auth*/ None),
        McpManager::new(Arc::new(plugins_manager_for_config(
            &config,
            auth_manager_from_optional_auth(/*auth*/ None),
        ))),
    ];
    for manager in managers {
        let servers = manager.runtime_servers(&config).await;
        assert!(!servers.contains_key(MAPLECODE_APPS_MCP_SERVER_NAME));
    }
    Ok(())
}

fn installed_manager(config: &Config, auth: Option<MapleCodeAuth>) -> McpManager {
    let mut builder = ExtensionRegistryBuilder::new();
    maplecode_mcp_extension::install(&mut builder);
    McpManager::new_with_extensions(
        Arc::new(plugins_manager_for_config(
            config,
            auth_manager_from_optional_auth(auth),
        )),
        Arc::new(builder.build()),
        maplecode_core::MapleCodeAppsToolsCache::default(),
    )
}

struct RemoveMapleCodeApps;

impl McpServerContributor<Config> for RemoveMapleCodeApps {
    fn id(&self) -> &'static str {
        "remove_maplecode_apps"
    }

    fn contribute<'a>(
        &'a self,
        _context: McpServerContributionContext<'a, Config>,
    ) -> maplecode_extension_api::ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            vec![McpServerContribution::Remove {
                name: MAPLECODE_APPS_MCP_SERVER_NAME.to_string(),
            }]
        })
    }
}
