# Maple integration investigation

Inspected release: Maple Code `rust-v0.170.0`, commit
`29eb79e9866b3262a777fdf8195c1b9e63684c91`, tree
`f657bf577be9e5dda06174066b9cbea2e6b8a02b`, latest non-prerelease returned by
GitHub on 2026-11-19. This directory initially contains **40 inspected upstream
files, not a full clone**. Shell DNS/network access prevented cloning. The
connected GitHub service supplied those files. Do not claim a baseline build.

## Upstream architecture and change boundary

| Exact path | Responsibility / intended change |
| --- | --- |
| `maplecode-rs/Cargo.toml` | Rust workspace; many crates, workspace dependencies and patched Git dependencies. Keep unchanged. |
| `maplecode-rs/cli/src/main.rs`, `maple_cmd.rs`, `Cargo.toml` | Native `maplecode` main, `arg0_dispatch_or_else`, then `dispatch_cli`, TUI or exec. Adds a separate `maple` command namespace for login/logout/status/models. |
| `maplecode-rs/model-provider-info/src/lib.rs` | Provider registry, auth configuration, endpoint construction. Add Maple with bearer env key, no OpenAI login or WebSockets. |
| `maplecode-rs/model-provider/src/models_endpoint.rs` | Provider/auth-bound discovery, network policy, retries, cache identity. Enable Maple discovery through this existing path. |
| `maplecode-rs/maplecode-api/src/endpoint/models.rs` | Fetch raw catalog and decode Maple Code metadata. Translate Maple metadata only for the Maple provider. |
| `maplecode-rs/models-manager/src/manager.rs`, `manager_tests.rs` | Add an explicit catalog read that propagates refresh errors for CLI diagnostics. Preserve existing conversation refresh/cache behavior and ownership checks. |
| `maplecode-rs/core/src/client.rs` | Build full Responses request; apply auth, policies, telemetry, retries, interceptors; send HTTP or WebSockets; map response stream. Preserve. |
| `maplecode-rs/maplecode-api/src/endpoint/responses.rs` | HTTP request boundary; translate Maple request before encoding, post chat/completions, adapt byte stream before upstream parser. |
| `maplecode-rs/maplecode-api/src/endpoint/session.rs` | Redact provider errors before request telemetry/retry callbacks see them, preserving status, timeout, network policy and retry advice. |
| `maplecode-rs/maplecode-api/src/maple.rs`, `maple_tests.rs`, `lib.rs`, `Cargo.toml` | Stream adapter, protocol dependency and native interface regression tests. |
| `maplecode-rs/config/src/config_toml.rs` | Reserve Maple identity; allow only non-secret base_url overrides. |
| `maplecode-rs/maplecode-api/src/sse/responses.rs` | Parses Responses events into ResponseEvent. Owns idle timeouts and cancels polling when receiver closes. Reuse unchanged. |
| `maplecode-rs/protocol/src/models.rs` | Canonical history and tool items. Preserve call IDs, names, namespaces and tool outputs. |
| `maplecode-rs/protocol/src/shell_environment.rs` | Add MAPLE_API_KEY to existing non-inheritable credentials list; prevent shell overrides from restoring it in tool children. |
| `maplecode-rs/core/src/thread_manager/model_catalog.rs` | Per-thread catalog and auth owner; preserves history during model changes. |
| `maplecode-rs/tui/src/slash_command.rs`, `chatwidget.rs` | Existing `/model` command, interactive conversation controls. Reuse existing selector. |
| `maplecode-rs/keyring-store/src/lib.rs` | Existing OS credential abstraction, reused unchanged. New `model-provider-info/src/maple_credentials.rs` validates endpoint-scoped keys and maps errors without leaking backend payloads. |
| `maplecode-cli/bin/maplecode.js`, `package.json` | Node launcher selects native platform package/vendor binary, forwards argv, environment and signals. Adapt only after agent validation. |
| `maple-protocol/` | Small independently testable JSON/SSE protocol adapter, path dependency of maplecode-api. No tool execution or permission decisions. |

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
`maplecode_models_manager::model_info::BASE_INSTRUCTIONS` on discovered models;
Maple metadata does not replace the coding-agent prompt. The existing TUI's
`chatwidget/model_popups.rs` emits `PersistModelSelection`, while
`session_model_selection.rs` dispatches `UpdateModel`/`ApplyModelSelection`.
These handlers are reused, with native flow verification still pending.

## Verified external contract

- https://maple.ai/docs/proxy/api: bearer authentication, `/v1/models`,
  `/v1/chat/completions`, SSE, optional reasoning/usage; **no Responses API**.
- https://maple.ai/docs/proxy/models: inspect `id`, `tasks`, `capabilities`,
  `context_window`; discovery does not prove account entitlement.
- The archived OpenSecretCloud proxy repository now points to
  https://github.com/MaplePrivacyLabs/Maple/tree/master/proxy. The inspected
  `services/opensecret/src/model_config.rs` confirms `capabilities.chat`,
  `tool_use`, `vision`, `reasoning`, `reasoning.supported_efforts` and
  `reasoning.default_effort`. Exact revision is in `upstream-source.json`.
  The running local proxy contract remains unverified.

Do not manufacture context sizes, model identifiers, reasoning levels or
capabilities. Missing metadata remains unknown. Native server tools, encrypted
history, uploaded files/audio and unsupported Responses items must fail clearly.
Function and custom tools require different argument representations; namespaces
need reversible names. Streaming tool arguments may interleave across indices.
No tool is executable until a complete valid call is received. A truncated stream
must never be reported as completed, and partial tool calls must not execute.

## Build, testing, licensing and implementation order

Read `docs/install.md`, `docs/contributing.md`, `LICENSE`, `NOTICE` and the pinned
toolchain. Upstream builds with `cargo build` in `maplecode-rs`, uses `just fmt`,
`just fix -p CRATE`, `just test -p CRATE` / nextest; routine runs avoid all-features.
The current contributing guide accepts issues, not external PRs. This local fork
does not push anything. Preserve Apache-2.0 and NOTICE in all distributions.

1. Baseline build/test before edits: attempted; unavailable workspace member
   `aws-auth/Cargo.toml` prevents even manifest loading. Clone failed resolving
   github.com. Record as environment/source-availability failures, not test failures.
2. Implement and test request/history/tool and SSE translation independently.
3. Wire provider, authenticated HTTP, catalog and stream integration into upstream.
4. Validate real authenticated chat and tool execution with proxy/key; neither
   is currently available. Do not treat mocks as real inference.
5. Implemented provider login/model commands before product branding; their
   compilation and OS integration are still blocked by the incomplete source.
6. Only after coding workflow validation: CLI/TUI branding, npm package changes
   and native release artifacts.

## Distribution investigation

The upstream CLI manifest names `maplecode-cli` and its main bin `maplecode` at
`cli/src/main.rs`. `maplecode-cli/bin/maplecode.js` maps Linux x64 to
`x86_64-unknown-linux-musl` / `@openai/codex-linux-x64`, first resolving an
optional platform package's `vendor`, then a bundled vendor directory. It
launches `vendor/<target>/bin/maplecode`, forwards argv/environment and process
signals, and annotates the installing package manager. A future Maple package
must adapt these platform package names and vendor paths together with the
native binary name and release scripts; renaming only the Node bin key is
insufficient. No package name has been reserved, no npm artifacts produced,
and no distribution or branding changes are applied before agent validation.

No architectural rewrites, replacement tool runner or sandbox changes are needed.
