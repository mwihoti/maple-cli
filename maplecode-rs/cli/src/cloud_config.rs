use anyhow::Context;
use anyhow::Result;
use maplecode_app_server::in_process::EmbeddedNetworkPolicy;
use maplecode_cloud_config::managed_config_bundle_loader_for_storage;
use maplecode_config::CloudConfigBundleLoader;
use maplecode_config::ConfigLoadOptions;
use maplecode_core::config::Config;
use maplecode_core::config::ConfigBuilder;
use maplecode_core::config::ConfigOverrides;
use maplecode_core::config::LoaderOverrides;
use maplecode_core::config::bootstrap_auth_config;
use maplecode_core::config::find_maplecode_home;
use maplecode_core::config::load_config_toml_with_layer_stack;
use maplecode_utils_absolute_path::AbsolutePathBuf;
use maplecode_utils_cli::CliConfigOverrides;

pub async fn load_config(
    config_overrides: &CliConfigOverrides,
    loader_overrides: LoaderOverrides,
) -> Result<Config> {
    let mut config = config_builder(
        config_overrides,
        loader_overrides,
        ConfigOverrides::default(),
    )
    .await?
    .build()
    .await
    .context("failed to load configuration")?;
    config.config_layer_stack.ensure_config_server_applied()?;
    EmbeddedNetworkPolicy::default().activate(&mut config);
    Ok(config)
}

pub async fn config_builder(
    config_overrides: &CliConfigOverrides,
    loader_overrides: LoaderOverrides,
    harness_overrides: ConfigOverrides,
) -> Result<ConfigBuilder> {
    let network_policy = EmbeddedNetworkPolicy::load(&loader_overrides).await;
    let cli_overrides = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let maplecode_home = find_maplecode_home().context("failed to resolve MAPLECODE_HOME")?;
    let cwd = match harness_overrides.cwd.as_deref() {
        Some(cwd) => AbsolutePathBuf::relative_to_current_dir(cwd),
        None => AbsolutePathBuf::current_dir(),
    }
    .context("failed to resolve current directory")?;
    let bootstrap_config = load_config_toml_with_layer_stack(
        maplecode_home.as_path(),
        Some(&cwd),
        cli_overrides.clone(),
        ConfigLoadOptions {
            loader_overrides: loader_overrides.clone(),
            strict_config: false,
            cloud_config_bundle: CloudConfigBundleLoader::default(),
        },
    )
    .await
    .context("failed to load bootstrap configuration")?;
    let cloud_config_bundle = managed_config_bundle_loader_for_storage(
        network_policy.bind_bootstrap_auth(
            bootstrap_auth_config(maplecode_home.as_path(), &bootstrap_config)
                .context("failed to resolve cloud configuration authentication")?,
        ),
        bootstrap_config.config_layer_stack.requirements_toml(),
        /*enable_maplecode_api_key_env*/ false,
    )
    .await
    .context("failed to initialize cloud configuration authentication")?;

    Ok(ConfigBuilder::default()
        .maplecode_home(maplecode_home.to_path_buf())
        .cli_overrides(cli_overrides)
        .loader_overrides(loader_overrides)
        .harness_overrides(harness_overrides)
        .cloud_config_bundle(cloud_config_bundle)
        .fallback_cwd(Some(cwd.to_path_buf())))
}
