//! Maple's Chat Completions wire protocol, below the agent/tool permission layer.

use crate::error::ApiError;
use futures::StreamExt;
use maple_protocol::request::Tool;
use maple_protocol::stream::Decoder;
use maplecode_client::ByteStream;
use maplecode_client::TransportError;
use std::collections::BTreeMap;

pub(crate) fn invalid(error: maple_protocol::Error) -> ApiError {
    ApiError::InvalidRequest {
        message: error.to_string(),
    }
}

/// Called before request telemetry so reflected credentials never reach it.
pub(crate) fn sanitize_transport(error: TransportError) -> TransportError {
    match error {
        TransportError::Http {
            status,
            retry_after,
            body,
            ..
        } => TransportError::Http {
            status,
            retry_after,
            url: None,
            headers: None,
            body: Some(
                serde_json::json!({"error":{
                    "message": maple_protocol::provider_error(status.as_u16(), body.as_deref()).to_string()
                }})
                .to_string(),
            ),
        },
        TransportError::Policy(policy) => TransportError::Policy(policy),
        TransportError::Timeout => TransportError::Timeout,
        _ => {
            TransportError::Network("Maple connection failed; check the proxy and base URL".into())
        }
    }
}

/// No new background tasks: dropping the upstream response receiver cancels
/// upstream polling, drops this decoder, and closes the Maple response body.
pub(crate) fn responses_stream(bytes: ByteStream, tools: BTreeMap<String, Tool>) -> ByteStream {
    Box::pin(futures::stream::try_unfold(
        (bytes, Decoder::new(tools)),
        |(mut bytes, mut decoder)| async move {
            loop {
                if decoder.is_done() {
                    return Ok(None);
                }
                let events = match bytes.next().await {
                    Some(Ok(chunk)) => decoder
                        .feed(&chunk)
                        .map_err(|error| TransportError::Build(error.to_string()))?,
                    Some(Err(TransportError::Policy(policy))) => {
                        return Err(TransportError::Policy(policy));
                    }
                    Some(Err(TransportError::Timeout)) => return Err(TransportError::Timeout),
                    Some(Err(_)) => {
                        return Err(TransportError::Network(
                            "Maple connection interrupted".into(),
                        ));
                    }
                    None => {
                        decoder
                            .eof()
                            .map_err(|error| TransportError::Build(error.to_string()))?;
                        return Ok(None);
                    }
                };
                if !events.is_empty() {
                    return Ok(Some((
                        maple_protocol::stream::encode_events(&events).into(),
                        (bytes, decoder),
                    )));
                }
            }
        },
    ))
}
