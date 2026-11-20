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

