# Validation record and remaining work

## Explicit model-discovery diagnostics follow-up

The user reported that the serial daemon rerun passed all 14 cases. The native
models command then returned a generic empty-catalog error despite the earlier
successful Python catalog request. The original cause of that host request has
not yet been established; credentials may differ between terminal sessions.

The explicit CLI command previously used a best-effort cache read that hid
refresh errors. It now preflights the configured credential and uses
`try_raw_model_catalog`, which propagates refresh errors. Existing conversation
reads retain their cache/fallback semantics. The manager's complete 63-test
suite passed, including three new regression tests for HTTP 401/403/429,
malformed catalog data, and stale cache reads after timeout. The native CLI and
test executables for codex-api, codex-cli, codex-model-provider-info,
codex-protocol and codex-models-manager rebuilt successfully offline (7m54s).
A missing-key check now returns an actionable credential-store/environment error
instead of claiming no models are available. Actual host model discovery and
inference remain to be verified.

Date: 2026-11-19. Full checkout: a sibling `maple-code-full` directory.
Branch: `feature/maple-provider`. Native target release: Codex `rust-v0.170.0`.

## Initial validation in the partial source checkout

| Check | Result |
| --- | --- |
| `git clone --depth 1 https://github.com/openai/codex.git maple-code` | Failed: shell could not resolve github.com. Connected GitHub reads supplied inspected source files instead. |
| Baseline `cargo build --offline --manifest-path codex-rs/Cargo.toml` | Blocked before compilation: missing workspace member `codex-rs/aws-auth/Cargo.toml`. This is a partial-source limitation, not an upstream test failure. |
| Baseline `cargo test --offline --manifest-path codex-rs/Cargo.toml -p codex-api` | Same manifest-loading blocker. No upstream baseline tests ran. |
| Modified native tests for codex-api, codex-model-provider-info, codex-protocol, codex-cli | Same blocker. These changes have not been type-checked as an integrated Codex workspace. |
| `cargo test --offline --manifest-path maple-protocol/Cargo.toml` | **22 tests passed**, plus empty unit/doc-test targets. Uses cached dependencies and installed Rust 1.99.1, not the unavailable pinned Rust 1.95.0. |
| `/usr/bin/cargo-clippy clippy --offline --manifest-path maple-protocol/Cargo.toml --all-targets -- -D warnings` | Passed for the standalone protocol crate. |
| `python -m unittest discover -s scripts/tests -v` | **7 tests passed**; request construction, auth-error redaction, redirect rejection, endpoint validation and a deterministic addition-tool probe. These are mocks, not authenticated inference. |
| System rustfmt on new/modified Rust sources | Passed. Rustup cargo-fmt was missing; the installed `/usr/bin/rustfmt` was used explicitly. |
| `git diff --check` | Passed. |
| Maple key presence | `MAPLE_API_KEY` absent. No credential values read or printed. |
| Proxy TCP connectivity | Socket creation/connect to 127.0.0.1:8080 returned `EPERM: Operation not permitted`. This session cannot determine whether the host proxy itself is running. |
| `python scripts/probe-maple.py models` | Exited with missing-key diagnostic before making a request. No real inference attempted. |
| Smoke fixture `cargo test --offline --manifest-path fixtures/maple-smoke/Cargo.toml` | Compiled; **2 tests failed as intentionally designed**, confirming the repair fixture's baseline. No agent repaired it. |

## Native test coverage

`codex-rs/codex-api/src/maple_tests.rs` uses the real upstream HTTP transport
trait and SSE parser interfaces, with memory-backed responses for:

- Valid/invalid bearer authentication, 403 access denial, 429 rate limits.
- Actual native SSE decoding of translated text and executable tool items.
- Preservation of call IDs; no executable item after interrupted responses.
- Model-catalog deserialization into upstream ModelInfo.
- Error-body/URL redaction **before** request telemetry.
- Idle timeout and receiver-drop cancellation releasing pending HTTP bodies.

Credential-store tests in `model-provider-info/src/maple_credentials.rs` cover
endpoint scoping, login/logout roundtrip, invalid key formatting and safe errors.
`protocol/src/shell_environment.rs` tests prevent Maple key inheritance or
restoration by shell-policy overrides and scrub explicit child-command overrides.
See the full-checkout results below.

Standalone Rust protocol tests cover roles/instructions, full history reload,
model changes, function/custom/namespaced/multiple calls, tool results, UTF-8 at
every byte split, LF/CRLF/CR framing, reasoning and usage, unknown/malformed
catalogs and context sizes, refusal/truncation, malformed JSON, invalid IDs,
unknown tools, safe provider diagnostics and cancelled partial output.
History roundtrip tests do not prove the complete TUI session-resume flow.

