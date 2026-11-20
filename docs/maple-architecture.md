# Maple integration investigation

Inspected release: OpenAI Codex `rust-v0.170.0`, commit
`29eb79e9866b3262a777fdf8195c1b9e63684c91`, tree
`f657bf577be9e5dda06174066b9cbea2e6b8a02b`, latest non-prerelease returned by
GitHub on 2026-11-19. This directory initially contains **40 inspected upstream
files, not a full clone**. Shell DNS/network access prevented cloning. The
connected GitHub service supplied those files. Do not claim a baseline build.

## Upstream architecture and change boundary

| Exact path | Responsibility / intended change |
| --- | --- |
| `codex-rs/Cargo.toml` | Rust workspace; many crates, workspace dependencies and patched Git dependencies. Keep unchanged. |
| `codex-rs/cli/src/main.rs`, `maple_cmd.rs`, `Cargo.toml` | Native `codex` main, `arg0_dispatch_or_else`, then `dispatch_cli`, TUI or exec. Adds a separate `maple` command namespace for login/logout/status/models. |
| `codex-rs/model-provider-info/src/lib.rs` | Provider registry, auth configuration, endpoint construction. Add Maple with bearer env key, no OpenAI login or WebSockets. |
| `codex-rs/model-provider/src/models_endpoint.rs` | Provider/auth-bound discovery, network policy, retries, cache identity. Enable Maple discovery through this existing path. |
| `codex-rs/codex-api/src/endpoint/models.rs` | Fetch raw catalog and decode Codex metadata. Translate Maple metadata only for the Maple provider. |
| `codex-rs/models-manager/src/manager.rs`, `manager_tests.rs` | Add an explicit catalog read that propagates refresh errors for CLI diagnostics. Preserve existing conversation refresh/cache behavior and ownership checks. |
| `codex-rs/core/src/client.rs` | Build full Responses request; apply auth, policies, telemetry, retries, interceptors; send HTTP or WebSockets; map response stream. Preserve. |
| `codex-rs/codex-api/src/endpoint/responses.rs` | HTTP request boundary; translate Maple request before encoding, post chat/completions, adapt byte stream before upstream parser. |
| `codex-rs/codex-api/src/endpoint/session.rs` | Redact provider errors before request telemetry/retry callbacks see them, preserving status, timeout, network policy and retry advice. |
| `codex-rs/codex-api/src/maple.rs`, `maple_tests.rs`, `lib.rs`, `Cargo.toml` | Stream adapter, protocol dependency and native interface regression tests. |
| `codex-rs/config/src/config_toml.rs` | Reserve Maple identity; allow only non-secret base_url overrides. |
| `codex-rs/codex-api/src/sse/responses.rs` | Parses Responses events into ResponseEvent. Owns idle timeouts and cancels polling when receiver closes. Reuse unchanged. |
| `codex-rs/protocol/src/models.rs` | Canonical history and tool items. Preserve call IDs, names, namespaces and tool outputs. |
| `codex-rs/protocol/src/shell_environment.rs` | Add MAPLE_API_KEY to existing non-inheritable credentials list; prevent shell overrides from restoring it in tool children. |
| `codex-rs/core/src/thread_manager/model_catalog.rs` | Per-thread catalog and auth owner; preserves history during model changes. |
| `codex-rs/tui/src/slash_command.rs`, `chatwidget.rs` | Existing `/model` command, interactive conversation controls. Reuse existing selector. |
| `codex-rs/keyring-store/src/lib.rs` | Existing OS credential abstraction, reused unchanged. New `model-provider-info/src/maple_credentials.rs` validates endpoint-scoped keys and maps errors without leaking backend payloads. |
| `codex-cli/bin/codex.js`, `package.json` | Node launcher selects native platform package/vendor binary, forwards argv, environment and signals. Adapt only after agent validation. |
| `maple-protocol/` | Small independently testable JSON/SSE protocol adapter, path dependency of codex-api. No tool execution or permission decisions. |

Lifecycle: CLI configuration -> provider/auth owner -> thread model catalog ->
core model client builds request with history and tools -> policy-aware HTTP
transport -> SSE parser -> ResponseEvent -> existing core tool router and
approval/sandbox layer -> tool result added to history -> next request. Resume
belongs to upstream rollout/thread storage; the adapter is stateless between
requests and preserves call identifiers in the canonical history.

The complete loop now lives in `core/src/session/turn.rs`: `run_turn`,
`run_sampling_request`, `try_run_sampling_request`, then `OutputItemDone`
handling. `core/src/tools/router.rs::build_tool_call` and dispatch methods
route real tool invocations. `core/src/tools/approvals.rs` owns approval policy
stages and managed human checkpoints. `sandboxing/src/lib.rs` exports platform
sandbox selection; none of these files are changed. Persistent history is
managed by `rollout/src/recorder.rs` (`record_canonical_items`,
`load_rollout_items`, `get_rollout_history`) and the existing thread store.

`WireApi` currently accepts only Responses and explicitly rejects `chat`.
Restoring an old chat implementation is inappropriate. The Maple branch will
translate at the HTTP boundary and retain Responses as the internal protocol.
Provider name `Maple` selects this adapter; ordinary Responses providers keep
their existing behavior. Maple WebSockets and remote compaction are disabled.
`model-provider/src/models_endpoint.rs` restores the existing generic
`codex_models_manager::model_info::BASE_INSTRUCTIONS` on discovered models;
Maple metadata does not replace the coding-agent prompt. The existing TUI's
`chatwidget/model_popups.rs` emits `PersistModelSelection`, while
`session_model_selection.rs` dispatches `UpdateModel`/`ApplyModelSelection`.
These handlers are reused, with native flow verification still pending.

