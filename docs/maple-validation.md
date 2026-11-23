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

## Unresolved gates

1. Repeat native validation with the pinned Rust 1.95.0 toolchain.
2. Validate Secret Service login/logout and terminal input restoration on Arch.
   There is no plaintext fallback. The current session cannot access system D-Bus.
3. Start the host Maple Proxy, supply a real key locally, fetch current models
   and establish an authenticated completion and actual tool-result roundtrip.
4. Run `scripts/smoke-agent.py` with the built native binary. Validate real file
   editing, command execution and the Git diff. Use the interactive CLI for
   approval prompts, cancellation, model selection and session resume.
5. After the coding workflow passes, change product defaults/branding to Maple
   Code, expose the final `maple-code` commands, adapt upstream Linux x64 npm
   native packaging, include notices, and verify local package installation.
   Package name, publishing, pushes and deployments remain unapproved/unperformed.

## Current compatibility limits

- Maple is a Chat Completions provider, not a Responses endpoint. Only the
  inspected built-in Maple identity selects translation; other providers retain
  their existing protocols.
- Only explicitly tool-capable chat catalog entries appear in agent selection.
  Older proxies lacking capability metadata fail clearly; account entitlement
  is determined by real requests, not catalog membership.
- Models with unknown context limits retain unknown metadata. Token estimates
  for Maple model families and automatic compaction need real validation;
  no tokenizer-specific guarantee is made.
- The native agent path sends streaming requests. Non-streaming completion
  decoding is tested independently; JSON returned to an SSE request fails
  rather than being fabricated into success.
- Function tools and JSON-wrapped raw custom tools are translated. Provider-side
  strict JSON-schema or custom grammar enforcement is not claimed; core still
  validates and authorizes tool execution. Incremental function arguments are
  buffered; upstream ignores function-argument preview events. Custom raw input
  is released once complete.
- Native hosted search, tool-search/deferred tools, uploaded-file/audio input,
  encrypted history, Responses structured output, WebSockets, server-side
  previous_response_id and remote compaction are unsupported. Unsupported
  requests return explicit errors. Existing local compaction remains upstream.
- Cross-provider resume of encrypted OpenAI history is unsupported. Maple-native
  history keeps plaintext reasoning and tool IDs; switching model compatibility
  still needs real requests.
- A valid `[DONE]` with a matching finish reason is required before any completed
  tool call is released. Malformed, filtered, truncated or disconnected streams
  do not report success. Frames/output have adapter memory bounds.
- The key is removed from model-reachable child environments using upstream's
  existing restricted-variable mechanism. This does not create a new OS-level
  secret isolation boundary against deliberately reading same-user process memory.

## Full-checkout follow-up

The host helper completed cloning and applied the overlay to
a sibling `maple-code-full` directory. Its baseline build and tests both exited 101:
the release lockfile still used internal package version 0.0.0 while the tagged
workspace used 0.170.0, and crates.io DNS resolution failed. Helper completion
did not mean a successful baseline build.

The current lockfile normalizes the 174 internal versions and adds the local
Maple protocol package. External dependency versions and checksums retain
their pinned values. `cargo fetch --locked --offline` now succeeds with the
populated cache. The current tests use installed Rust 1.99.1.

All 22 standalone protocol tests and seven Python probe tests passed. The
seven native `maple_tests` passed with an in-memory transport, as did both
credential-store tests and both shell-environment tests. The smoke harness
passed with a test executable that made a deterministic fixture edit and
confirmed that the baseline source was staged.

The workspace test build produced `codex-rs/target/debug/codex`. The executable
reported version 0.170.0, displayed the Maple subcommands, and recognized a
fixture environment key in `maple auth status`. The provider-info suite passed
46 tests, and the protocol suite passed 381. The API suite passed 196 tests,
including the Maple tests. Its remaining 17 tests hit the sandbox's socket
restriction when starting mock servers.

The model-provider and CLI unit suites showed the same socket restriction.
The model-provider suite passed 101 tests and had 37 socket failures. The two
CLI targets passed 299 tests, had 12 socket failures, and ignored one test.

The user's terminal successfully retrieved Maple's authenticated model catalog.
That confirms catalog access from the host; model inference and the coding
workflow remain unverified.

## Native build and temporary-storage follow-up

The host has now compiled the integrated native CLI. The existing binary reports
`codex-cli 0.170.0`, and `codex maple --help` exposes the provider commands.
The earlier uncompiled-state notes above describe the initial environment and
are superseded by this update for the full checkout.

Executed directly from the host-built test executables, without a Cargo rebuild:

- codex-api `maple_tests`: **7 passed** (real upstream parser/transport interfaces,
  with deterministic memory responses).
- model-provider-info `maple_credentials::tests`: **2 passed**.
- protocol `maple_key_tests`: **2 passed**.

These 11 native tests do not establish live Maple inference or OS keyring access.

The CLI daemon suite's 14 reported failures occurred while constructing test
fixtures: each concurrent test tried to copy a 1269 MiB debug CLI onto the
quota-enabled /tmp filesystem. Fixture setup hardcodes /tmp, so TMPDIR does not
redirect it. The source filesystem has free space; no files or caches were deleted.

A separate stripped CLI fixture was prepared at
`codex-rs/target/maple-test-bin/codex` (427 MiB), and its version was verified.
The original debug executable remains unchanged. Run
`python scripts/test-daemon-low-disk.py` from the full checkout to reuse the
compiled daemon test executable, select that compact fixture through upstream's
existing CARGO_BIN_EXE_codex hook, and execute tests serially. Preparation was
verified here; the daemon suite still requires a host-terminal rerun because this
agent environment restricts local socket access. No tests are skipped by the helper.

## Live Maple validation and fixes

Date: 2026-11-20. Maple Proxy 0.3.2 (`ghcr.io/opensecretcloud/maple-proxy`),
debug CLI built from this branch.

- `codex maple models` returned nine tool-capable models through the proxy.
- `codex exec` with `deepseek-v4-1-flash` and `glm-5-3-flash` completed turns,
  including a real `ls` tool call whose result the model summarized.

Three defects were found against the live service and fixed:

- Every turn failed with "hosted or deferred tools are unsupported" because the
  default cached web search adds a hosted `web_search` tool. The Maple provider
  now reports no hosted web search or image generation.
- The proxy opens each stream with `{"role":"assistant","content":""}`. The
  adapter emitted that empty delta before `response.output_item.added`, which
  the debug CLI treats as a protocol violation. Empty text deltas are skipped.
- The catalog request had a 5 second deadline, but the proxy's first request
  after start or idle includes an attestation handshake measured at 7-14
  seconds. The Maple catalog deadline is now 45 seconds.

Measured single-request latency for a one-sentence prompt, low versus high
`reasoning_effort`: gpt-oss-120b 2.1 s / 5.1 s, kimi-k3-1-pro 2.9 s / 5.8 s,
glm-5-3 2.4 s / 4.9 s, deepseek-v4-1-flash 1.9 s / 2.5 s. These are single
samples, not benchmarks.

For roughly ten minutes during testing every request returned 502 with
"PCR0 is not approved by the configured trust policy" from both the container
and the extracted proxy binary; it cleared without local changes.

A later session failed with "encrypted, audio, uploaded-file or non-user image
content is unsupported" after the model called `view_image` on a screenshot.
Tool-result images are now moved into a user message that follows the tool
batch. A direct request in that shape returned HTTP 200 from
`deepseek-v4-1-flash-1017-uncensored`, and `codex exec` identified the colour
of a solid test image through `view_image`.

`--full-access` was added as a visible alias of
`--dangerously-bypass-approvals-and-sandbox`. Only argument parsing is tested;
no live session was run in that mode.

With the Maple provider the TUI header reads "Maple Code", the composer
placeholder reads "Ask Maple Code to do anything", and `exec` prints
"Maple Code v<version>"; these were checked against the debug build. Approval
messages use "maple-code" and are covered by a type check only.

A session ended with "model requested an unadvertised tool". The name the
model used was not captured. The decoder now resolves readable tool names to
the single advertised tool they identify and turns any other name into an
undispatchable `unavailable_tool` call. This is covered by decoder tests; the
original failure was not reproduced live. A normal `codex exec` tool call
still completed on the rebuilt binary.

More interface text follows the provider: `/` command descriptions, the
session help lines, the exit menu, resume hints and the full-access warning.
The `/permissions` and `/quit` descriptions were seen reading "Maple Code" in a
live session; the rest were type-checked only.

On 2026-11-21 every request returned 502 with "PCR0 is not approved by the
configured trust policy" for over twenty minutes; retries and a proxy restart
did not help. Proxy 0.3.2 came from the archived OpenSecretCloud repository.
Replacing it with the v3.4.9 proxy from MaplePrivacyLabs/Maple releases
(checksum verified) restored the model list and a `codex exec` turn at once.
The earlier ten-minute outage on 2026-11-20 was probably the same cause.

`scripts/install-from-source.sh` was syntax-checked with `bash -n`, and its
proxy download step was run on Linux x86_64 into a temporary directory.

On 2026-11-22 the remaining interface text in the TUI crate was moved to the
provider-dependent wording: menus, notices, errors and the permissions picker.
It passed `cargo check -p codex-tui`. The binary could not be relinked because
the disk was full, so none of this text has been seen in a live session yet.
Still named Codex: Windows-only prompts, sign-in onboarding, a few constants,
text sent to the model, and messages produced by other crates.

On 2026-11-23, after disk space was freed, the binary was rebuilt with
`CARGO_INCREMENTAL=0`. The full-access description in `/permissions` is present
in the binary as "Use with caution: Maple Code can edit files ...". The other
text moved on 2026-11-22 is now built but has still not been read on screen,
apart from the header, the placeholder and the startup help lines.

A Maple session used to open as "OpenAI Codex" with the OpenAI logo and
"Ask Codex to do anything" for about a second, because the first frame was
drawn before the provider was known. The name is now set from the bootstrap
config, and the logo is replaced by a maple leaf for the Maple provider. The
session was started in a 110x40 terminal and captured every 0.3 seconds: every
frame read "Maple Code", and the first drawn frame showed the leaf. Only a
provider set by `model_provider` in `config.toml` was tried.

Still unverified: OS keyring login, session resume, MCP tools exposed through
deferred tool search, images on models without vision, and the release build.

