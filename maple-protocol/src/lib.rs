//! Pure protocol translation. This crate cannot run tools or grant permissions.
//! Diagnostics intentionally contain no provider payloads, URLs or credentials.

pub mod catalog;
pub mod request;
pub mod stream;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(pub &'static str);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Maple: {}", self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error("missing or invalid required string"))
}

/// Do not include server error bodies: they can echo Authorization headers.
pub fn http_error(status: u16) -> Error {
    Error(match status {
        401 => "authentication failed; check MAPLE_API_KEY",
        403 => "access denied; check model entitlement, plan and quota",
        404 => "endpoint or model unavailable; refresh the catalog and check MAPLE_BASE_URL",
        408 | 504 => "request timed out",
        413 => "request exceeds proxy body limits",
        429 => "rate limit or quota reached; retry later or check account usage",
        500..=599 => "provider temporarily unavailable",
        _ => "provider rejected the request",
    })
}

/// Interpret only recognized machine codes. Never copy provider messages.
pub fn provider_error(status: u16, body: Option<&str>) -> Error {
    let parsed = body
        .filter(|s| s.len() <= 1024 * 1024)
        .and_then(|s| serde_json::from_str::<Value>(s).ok());
    let code = parsed
        .as_ref()
        .and_then(|v| v.pointer("/error/code"))
        .and_then(Value::as_str);
    match code {
        Some("context_length_exceeded" | "context_window_exceeded") => Error(
            "model context limit exceeded; compact the conversation or choose a larger-context model",
        ),
        Some("invalid_api_key" | "authentication_error") => http_error(401),
        Some("model_not_found" | "model_access_denied") => Error(
            "model is unavailable or restricted by the account; refresh models and check entitlement",
        ),
        Some("insufficient_quota" | "usage_limit_reached" | "free_tier_token_limit_exceeded") => {
            Error("account usage limit reached; check plan and remaining credits")
        }
        Some("unsupported_model_capabilities" | "unsupported_parameter") => Error(
            "model does not support a requested capability or parameter; choose a compatible model",
        ),
        _ => http_error(status),
    }
}
