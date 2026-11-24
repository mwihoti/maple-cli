use maplecode_api::ImageEditRequest;
use maplecode_api::ImageGenerationRequest;
use maplecode_api::ImageRequestError;
use maplecode_api::ImageResponse;
use maplecode_api::ImagesClient;
use maplecode_api::ReqwestTransport;
use maplecode_api::map_api_error;
use maplecode_core::X_MAPLECODE_TURN_METADATA_HEADER;
use maplecode_http_client::ClientRouteClass;
use maplecode_http_client::HttpClientFactory;
use maplecode_login::default_client::add_originator_header;
use maplecode_login::default_client::create_transport_for_routes_async;
use maplecode_model_provider::SharedModelProvider;
use maplecode_protocol::error::MapleCodeErr;
use http::HeaderMap;
use http::HeaderValue;

const X_MAPLECODE_IMAGE_TURN_ID_HEADER: &str = "x-maplecode-image-turn-id";

pub(crate) struct ImageBackendError {
    message: String,
    maplecode_error: MapleCodeErr,
    imagegen_request_id: Option<String>,
}

impl ImageBackendError {
    fn from_image_request(error: ImageRequestError) -> Self {
        let (error, imagegen_request_id) = error.into_parts();
        let message = error.to_string();
        Self {
            message,
            maplecode_error: map_api_error(error),
            imagegen_request_id,
        }
    }

    fn from_message(message: String) -> Self {
        Self {
            maplecode_error: MapleCodeErr::Stream(message.clone()),
            message,
            imagegen_request_id: None,
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn maplecode_error(&self) -> &MapleCodeErr {
        &self.maplecode_error
    }

    pub(crate) fn imagegen_request_id(&self) -> Option<&str> {
        self.imagegen_request_id.as_deref()
    }
}

#[derive(Clone)]
pub(crate) struct MapleCodeImagesBackend {
    provider: SharedModelProvider,
    http_client_factory: HttpClientFactory,
    originator: Option<String>,
}

impl MapleCodeImagesBackend {
    /// Creates a backend that sends image requests through the active model provider.
    pub(crate) fn new(
        provider: SharedModelProvider,
        http_client_factory: HttpClientFactory,
        originator: Option<String>,
    ) -> Self {
        Self {
            provider,
            http_client_factory,
            originator,
        }
    }

    /// Resolves the provider and auth required for the current image API request.
    async fn client(&self) -> Result<ImagesClient<ReqwestTransport>, ImageBackendError> {
        let setup = maplecode_model_provider::resolve_request(
            self.provider.as_ref(),
            /*scope*/ None,
            /*routing*/ None,
            self.http_client_factory.clone(),
        )
        .await
        .map_err(|err| ImageBackendError::from_message(err.to_string()))?;
        let transport = create_transport_for_routes_async(
            setup.http_client_factory,
            ClientRouteClass::Api,
            setup.redirect_policy,
        )
        .await
        .map_err(|err| ImageBackendError::from_message(err.to_string()))?;
        Ok(ImagesClient::new(
            transport,
            setup.api_provider,
            setup.api_auth,
        ))
    }

    /// Sends a standalone image generation request through the configured Images client.
    pub(crate) async fn generate(
        &self,
        request: ImageGenerationRequest,
        turn_id: &str,
        maplecode_turn_metadata: Option<&str>,
    ) -> Result<(ImageResponse, Option<String>), ImageBackendError> {
        self.client()
            .await?
            .generate(
                &request,
                image_request_headers(self.originator.as_deref(), turn_id, maplecode_turn_metadata),
            )
            .await
            .map_err(ImageBackendError::from_image_request)
    }

    /// Sends a standalone image edit request through the configured Images client.
    pub(crate) async fn edit(
        &self,
        request: ImageEditRequest,
        turn_id: &str,
        maplecode_turn_metadata: Option<&str>,
    ) -> Result<(ImageResponse, Option<String>), ImageBackendError> {
        self.client()
            .await?
            .edit(
                &request,
                image_request_headers(self.originator.as_deref(), turn_id, maplecode_turn_metadata),
            )
            .await
            .map_err(ImageBackendError::from_image_request)
    }
}

fn image_request_headers(
    originator: Option<&str>,
    turn_id: &str,
    maplecode_turn_metadata: Option<&str>,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(turn_id) = HeaderValue::from_str(turn_id) {
        headers.insert(X_MAPLECODE_IMAGE_TURN_ID_HEADER, turn_id);
    }
    if let Some(originator) = originator {
        add_originator_header(&mut headers, originator);
    }

    if let Some(metadata) = maplecode_turn_metadata
        && let Ok(metadata) = HeaderValue::from_str(metadata)
    {
        headers.insert(X_MAPLECODE_TURN_METADATA_HEADER, metadata);
    }

    headers
}
