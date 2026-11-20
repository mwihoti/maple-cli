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

#[tokio::test]
async fn idle_timeout_fails_and_releases_the_http_body() {
    let mut transport = MemoryTransport::new(vec![]);
    transport.hold_open = true;
    let dropped = Arc::clone(&transport.dropped);
    let client = ResponsesClient::new(transport, provider(), Arc::new(KeyAuth(true)));
    let events = tokio::time::timeout(Duration::from_secs(2), async {
        client
            .stream(request(), HeaderMap::new(), Compression::None, None)
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await
    })
    .await
    .unwrap();
    assert!(events.iter().any(Result::is_err));
    assert!(dropped.load(Ordering::SeqCst));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Ok(ResponseEvent::Completed { .. })))
    );
}

#[tokio::test]
async fn cancellation_drops_pending_provider_stream_without_executing_tools() {
    let mut transport = MemoryTransport::new(vec![]);
    transport.hold_open = true;
    let dropped = Arc::clone(&transport.dropped);
    let client = ResponsesClient::new(transport, provider(), Arc::new(KeyAuth(true)));
    let stream = client
        .stream(request(), HeaderMap::new(), Compression::None, None)
        .await
        .unwrap();
    drop(stream);
    tokio::time::timeout(Duration::from_secs(2), async {
        while !dropped.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

struct KeyAuth(bool);
impl AuthProvider for KeyAuth {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), crate::auth::AuthError> {
        headers.insert(
            http::header::AUTHORIZATION,
            if self.0 {
                "Bearer fixture-key"
            } else {
                "Bearer invalid-fixture"
            }
            .parse()
            .unwrap(),
        );
        Ok(())
    }
}

fn provider() -> Provider {
    Provider {
        name: "Maple".into(),
        base_url: "http://127.0.0.1:8080/v1".into(),
        query_params: None,
        headers: HeaderMap::new(),
        retry: RetryConfig {
            max_attempts: 1,
            base_delay: Duration::from_millis(1),
            retry_429: false,
            retry_5xx: false,
            retry_transport: false,
        },
        stream_idle_timeout: Duration::from_millis(100),
    }
}

fn request() -> Value {
    json!({"model":"fixture","input":[{"type":"message","role":"user","content":"Fix this"}],
        "tools":[{"type":"function","name":"exec_command","parameters":{"type":"object"}}],"tool_choice":"auto"})
}

fn frame(delta: Value, reason: Value) -> Vec<u8> {
    format!(
        "data: {}\n\n",
        json!({"id":"fixture-response","choices":[{
        "index":0,"delta":delta,"finish_reason":reason}]})
    )
    .into_bytes()
}

#[tokio::test]
async fn authenticated_maple_request_streams_through_real_upstream_parser() {
    let mut body = frame(json!({"content":"Done 🍁"}), json!("stop"));
    body.extend(b"data: [DONE]\n\n");
    let transport = MemoryTransport::new(body);
    let client = ResponsesClient::new(transport.clone(), provider(), Arc::new(KeyAuth(true)));
    let events = client
        .stream(request(), HeaderMap::new(), Compression::None, None)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Ok(ResponseEvent::OutputTextDelta(s)) if s=="Done 🍁"))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Ok(ResponseEvent::Completed { .. })))
    );
    let captured = transport.captured.lock().unwrap();
    assert_eq!(captured[0].url, "http://127.0.0.1:8080/v1/chat/completions");
    assert_eq!(captured[0].method, Method::POST);
    let Some(RequestBody::EncodedJson(body)) = &captured[0].body else {
        panic!("expected encoded chat body")
    };
    let body: Value = serde_json::from_slice(body.as_bytes()).unwrap();
    assert!(body.get("messages").is_some());
    assert!(body.get("input").is_none());
}

#[tokio::test]
async fn invalid_key_and_account_restrictions_preserve_status_and_redact_body() {
    for status in [
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::TOO_MANY_REQUESTS,
    ] {
        let mut transport = MemoryTransport::new(vec![]);
        transport.status = status;
        let client = ResponsesClient::new(
            transport,
            provider(),
            Arc::new(KeyAuth(status != StatusCode::UNAUTHORIZED)),
        );
        let error = client
            .stream(request(), HeaderMap::new(), Compression::None, None)
            .await
            .err()
            .unwrap();
        assert!(!format!("{error:?}").contains("fixture-key"));
        let crate::error::ApiError::Transport(TransportError::Http {
            status: actual,
            url,
            headers,
            ..
        }) = error
        else {
            panic!("HTTP status lost")
        };
        assert_eq!(actual, status);
        assert!(url.is_none());
        assert!(headers.is_none());
    }
}

#[tokio::test]
async fn tool_calls_become_executable_upstream_response_items_only_after_terminal_frame() {
    let r = request();
    let translated = maple_protocol::request::translate(&r).unwrap();
    let name = translated.body["tools"][0]["function"]["name"].clone();
    for terminal in [false, true] {
        let mut body = frame(
            json!({"tool_calls":[{"index":0,"id":"call-real","type":"function",
            "function":{"name":name,"arguments":"{\"cmd\":\"cargo test\"}"}}]}),
            json!("tool_calls"),
        );
        if terminal {
            body.extend(b"data: [DONE]\n\n");
        }
        let client = ResponsesClient::new(
            MemoryTransport::new(body),
            provider(),
            Arc::new(KeyAuth(true)),
        );
        let events = client
            .stream(r.clone(), HeaderMap::new(), Compression::None, None)
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await;
        assert_eq!(events.iter().any(|e|matches!(e,Ok(ResponseEvent::OutputItemDone(
            codex_protocol::models::ResponseItem::FunctionCall{name,call_id,..})) if name=="exec_command" && call_id=="call-real")),terminal);
        assert_eq!(events.iter().any(Result::is_err), !terminal);
    }
}

#[tokio::test]
async fn real_models_decoder_accepts_maple_catalog_without_inventing_context() {
    let body = serde_json::to_vec(
        &json!({"data":[{"id":"new-fixture","display_name":"New Fixture",
        "tasks":["generate"],"capabilities":{"chat":true,"tool_use":true}}]}),
    )
    .unwrap();
    let client = ModelsClient::new(
        MemoryTransport::new(body),
        provider(),
        Arc::new(KeyAuth(true)),
    );
    let (models, _) = client
        .list_models(
            "http://127.0.0.1:8080/v1/models".into(),
            HeaderMap::new(),
            Some(1024 * 1024),
        )
        .await
        .unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].slug, "new-fixture");
    assert_eq!(models[0].context_window, None);
    assert!(!models[0].supports_reasoning_summary_parameter);
}
