//! Integration tests using the real upstream transport and SSE parser interfaces.
//! These tests require the complete Codex workspace; no network or credentials.

use crate::auth::AuthProvider;
use crate::common::ResponseEvent;
use crate::endpoint::models::ModelsClient;
use crate::endpoint::responses::ResponsesClient;
use crate::provider::{Provider, RetryConfig};
use crate::requests::Compression;
use codex_client::{HttpTransport, Request, RequestBody, Response, StreamResponse, TransportError};
use futures::StreamExt;
use http::{HeaderMap, Method, StatusCode};
use serde_json::{Value, json};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

#[derive(Clone)]
struct MemoryTransport {
    captured: Arc<Mutex<Vec<Request>>>,
    body: Vec<u8>,
    status: StatusCode,
    hold_open: bool,
    dropped: Arc<AtomicBool>,
}

impl MemoryTransport {
    fn new(body: Vec<u8>) -> Self {
        Self {
            captured: Arc::default(),
            body,
            status: StatusCode::OK,
            hold_open: false,
            dropped: Arc::default(),
        }
    }
    fn capture(&self, request: Request) -> Result<(), TransportError> {
        let authorized = request
            .headers
            .get(http::header::AUTHORIZATION)
            .is_some_and(|v| v == "Bearer fixture-key");
        self.captured.lock().unwrap().push(request);
        if !authorized || !self.status.is_success() {
            return Err(TransportError::Http {
                status: if authorized {
                    self.status
                } else {
                    StatusCode::UNAUTHORIZED
                },
                url: Some("http://localhost/fixture-key".into()),
                headers: None,
                body: Some("fixture-key: must never reach diagnostics".into()),
                retry_after: None,
            });
        }
        Ok(())
    }
}

impl HttpTransport for MemoryTransport {
    async fn execute(&self, request: Request) -> Result<Response, TransportError> {
        self.capture(request)?;
        Ok(Response {
            status: self.status,
            headers: HeaderMap::new(),
            body: self.body.clone().into(),
        })
    }
    async fn stream(&self, request: Request) -> Result<StreamResponse, TransportError> {
        self.capture(request)?;
        if self.hold_open {
            return Ok(StreamResponse {
                status: self.status,
                headers: HeaderMap::new(),
                bytes: Box::pin(HoldOpen(Arc::clone(&self.dropped))),
            });
        }
        // Split through every UTF-8 character, SSE delimiter and argument fragment.
        let chunks = self
            .body
            .iter()
            .map(|b| Ok(vec![*b].into()))
            .collect::<Vec<_>>();
        Ok(StreamResponse {
            status: self.status,
            headers: HeaderMap::new(),
            bytes: Box::pin(futures::stream::iter(chunks)),
        })
    }
}

struct HoldOpen(Arc<AtomicBool>);
impl futures::Stream for HoldOpen {
    type Item = Result<bytes::Bytes, TransportError>;
    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Pending
    }
}
impl Drop for HoldOpen {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct Telemetry(Mutex<Vec<String>>);
impl codex_client::RequestTelemetry for Telemetry {
    fn on_request(
        &self,
        _attempt: u64,
        _status: Option<StatusCode>,
        error: Option<&TransportError>,
        _duration: Duration,
    ) {
        self.0.lock().unwrap().push(format!("{error:?}"));
    }
}

#[tokio::test]
async fn provider_errors_are_redacted_before_request_telemetry() {
    let mut transport = MemoryTransport::new(vec![]);
    transport.status = StatusCode::UNAUTHORIZED;
    let telemetry = Arc::new(Telemetry::default());
    let client = ResponsesClient::new(transport, provider(), Arc::new(KeyAuth(false)))
        .with_telemetry(Some(telemetry.clone()), None);
    assert!(
        client
            .stream(request(), HeaderMap::new(), Compression::None, None)
            .await
            .is_err()
    );
    let messages = telemetry.0.lock().unwrap();
    assert_eq!(messages.len(), 1);
    assert!(!messages[0].contains("fixture-key"));
    assert!(!messages[0].contains("localhost"));
}
