use maplecode_backend_client::Client as BackendClient;
use maplecode_backend_client::ConfigBundleResponse;
use maplecode_backend_client::DeliveredTomlFragment;
use maplecode_config::CloudConfigBundle;
use maplecode_config::CloudConfigFragment;
use maplecode_config::CloudConfigTomlBundle;
use maplecode_config::CloudRequirementsFragment;
use maplecode_config::CloudRequirementsTomlBundle;
use maplecode_http_client::HttpClientFactory;
use maplecode_login::MaplecodeAuth;
use std::future::Future;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetryableFailureKind {
    Request { status_code: Option<u16> },
    AuthRecovery { status_code: Option<u16> },
}

impl RetryableFailureKind {
    pub(crate) fn status_code(self) -> Option<u16> {
        match self {
            Self::Request { status_code } | Self::AuthRecovery { status_code } => status_code,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BundleRequestError {
    Policy(maplecode_http_client::NetworkPolicyDenied),
    Fatal {
        error: maplecode_config::CloudConfigBundleLoadError,
        metric_reason: Option<&'static str>,
    },
    Retryable(RetryableFailureKind),
    Unauthorized {
        status_code: Option<u16>,
        message: String,
    },
}

impl BundleRequestError {
    pub(crate) fn auth_failure(
        status_code: Option<u16>,
        message: impl Into<String>,
        metric_reason: Option<&'static str>,
    ) -> Self {
        Self::Fatal {
            error: maplecode_config::CloudConfigBundleLoadError::new(
                maplecode_config::CloudConfigBundleLoadErrorCode::Auth,
                status_code,
                message,
            ),
            metric_reason,
        }
    }
}

/// Retrieves one cloud config bundle from the backend.
///
/// Implementations should return the backend-selected bundle exactly as delivered and leave
/// validation, caching, and config/requirements parsing decisions to the service layer.
pub(crate) trait BundleClient: Send + Sync {
    fn get_bundle(
        &self,
        auth: &MaplecodeAuth,
    ) -> impl Future<Output = Result<CloudConfigBundle, BundleRequestError>> + Send;
}

pub(crate) struct BackendBundleClient {
    base_url: String,
    http_client_factory: HttpClientFactory,
}

impl BackendBundleClient {
    pub(crate) fn new(base_url: String, http_client_factory: HttpClientFactory) -> Self {
        Self {
            base_url,
            http_client_factory,
        }
    }
}

impl BundleClient for BackendBundleClient {
    async fn get_bundle(
        &self,
        auth: &MaplecodeAuth,
    ) -> Result<CloudConfigBundle, BundleRequestError> {
        let client = BackendClient::from_auth(
            self.base_url.clone(),
            auth,
            self.http_client_factory.clone(),
        );

        fetch_bundle(&client).await
    }
}

pub(crate) async fn fetch_bundle(
    client: &BackendClient,
) -> Result<CloudConfigBundle, BundleRequestError> {
    client
        .get_config_bundle()
        .await
        .map(bundle_from_response)
        .map_err(bundle_request_error)
}

fn bundle_request_error(error: maplecode_backend_client::RequestError) -> BundleRequestError {
    if let maplecode_backend_client::RequestError::Policy(denied) = &error {
        return BundleRequestError::Policy(*denied);
    }
    let status_code = error.status().map(|status| status.as_u16());
    if error.is_unauthorized() {
        BundleRequestError::Unauthorized {
            status_code,
            message: "Managed configuration sign-in is required".into(),
        }
    } else {
        BundleRequestError::Retryable(RetryableFailureKind::Request { status_code })
    }
}

pub(crate) fn bundle_from_response(response: ConfigBundleResponse) -> CloudConfigBundle {
    let config_toml = response
        .config_toml
        .flatten()
        .map(|config_toml| *config_toml)
        .and_then(|config_toml| config_toml.enterprise_managed.flatten())
        .unwrap_or_default()
        .into_iter()
        .map(config_fragment_from_delivered)
        .collect();
    let requirements_toml = response
        .requirements_toml
        .flatten()
        .map(|requirements_toml| *requirements_toml)
        .and_then(|requirements_toml| requirements_toml.enterprise_managed.flatten())
        .unwrap_or_default()
        .into_iter()
        .map(requirements_fragment_from_delivered)
        .collect();

    CloudConfigBundle {
        config_toml: CloudConfigTomlBundle {
            enterprise_managed: config_toml,
        },
        requirements_toml: CloudRequirementsTomlBundle {
            enterprise_managed: requirements_toml,
        },
    }
}

fn config_fragment_from_delivered(fragment: DeliveredTomlFragment) -> CloudConfigFragment {
    CloudConfigFragment {
        id: fragment.id,
        name: fragment.name,
        contents: fragment.contents,
    }
}

fn requirements_fragment_from_delivered(
    fragment: DeliveredTomlFragment,
) -> CloudRequirementsFragment {
    CloudRequirementsFragment {
        id: fragment.id,
        name: fragment.name,
        contents: fragment.contents,
    }
}
