# Maple Code — integration in progress

This is the full Codex source checkout with the Maple provider changes applied.
The Maple protocol adapter and native provider tests pass with cached Cargo
dependencies. Live inference and the coding workflow still require access to
the local Maple Proxy. The [validation record](docs/maple-validation.md)
tracks build and test results.

Work is on `feature/maple-provider`, based on inspected Codex `rust-v0.170.0`
(commit `29eb79e9866b3262a777fdf8195c1b9e63684c91`). Original Apache-2.0
[LICENSE](LICENSE) and [NOTICE](NOTICE) are preserved. Nothing has been pushed
or published. See [architecture](docs/maple-architecture.md),
[validation and limitations](docs/maple-validation.md), and the exact source
provenance in [upstream-source.json](upstream-source.json).

## Implemented

- Built-in `maple` provider, default `http://127.0.0.1:8080/v1`, configurable
  with `MAPLE_BASE_URL` or a non-secret provider base URL override.
- Bearer authentication from `MAPLE_API_KEY`, with endpoint-scoped OS credential
  storage as fallback. No plaintext credential storage.
- Chat Completions request/history adapter, namespaced JSON function calls and
  custom raw-input tools such as apply_patch. Existing core runs all tools and
  retains its approvals, sandbox and Git behavior.
- Incremental SSE translation into upstream Responses events; text, reasoning,
  usage and interleaved tool arguments; incomplete calls never execute.
- Dynamic `/models` conversion using actual Maple capabilities/context data.
  Non-tool models and unknown tool capabilities are excluded from the agent
  catalog. Discovery does not prove account access.
- Provider commands under `codex maple`: secure login, logout, auth status,
  and model listing with `--search` and `--json`.
- Existing `--model`, `/model`, model persistence and resume architecture reused.
- Credential redaction before HTTP telemetry and prevention of MAPLE_API_KEY
  inheritance by model-reachable child processes.

Real Secret Service login, interactive model selection and session resume
still need validation. Product branding and npm packaging follow the live
coding smoke test.

## Run what is testable now

```bash
cd /path/to/maple-code-full
cargo test --locked --offline --target-dir /tmp/maple-protocol-target --manifest-path maple-protocol/Cargo.toml
python -m unittest discover -s scripts/tests -v
```

## Build and test the native CLI

Upstream pins Rust 1.95.0. See the [upstream build instructions](docs/install.md).
After `cargo fetch --locked` succeeds, you can add `--offline` to use cached
dependencies.

```bash
cd /path/to/maple-code-full/codex-rs
cargo test --locked -p codex-api -p codex-model-provider-info -p codex-model-provider -p codex-protocol -p codex-cli
cargo build --locked --bin codex
```

The original upstream build attempts are recorded in `maple-baseline-results.json`.
The current lockfile includes the Maple protocol crate and the internal version
changes required by the tagged source.

## Authenticate and discover models after the native build passes

Start Maple Proxy on the same computer. Keep the actual API key out of chat,
shell arguments, config files and Git. Use the hidden prompt and OS store:

```bash
cd /path/to/maple-code-full
./codex-rs/target/debug/codex maple login
./codex-rs/target/debug/codex maple auth status
./codex-rs/target/debug/codex maple models
./codex-rs/target/debug/codex maple models --search 'your search'
```

Login stores the key; it does not claim the server accepted it. Model discovery
verifies the key/catalog request. An actual completion is still necessary to
verify account access to a selected model. On Linux, an unlocked Secret Service
credential store is required for stored credentials. If unavailable, securely
set the environment variable in your own terminal:

```bash
read -rsp 'Maple API key: ' MAPLE_API_KEY
export MAPLE_API_KEY
export MAPLE_BASE_URL=http://127.0.0.1:8080/v1
```

`MAPLE_API_KEY` takes precedence over stored credentials. `codex maple logout`
deletes the current endpoint's stored key; also `unset MAPLE_API_KEY` to stop
using an environment key. Auth status reports configuration, not verified access.
Changing the base URL changes the credential-store account.

Choose an exact tool-capable model ID from the live list; no model ID is assumed:

```bash
read -rp 'Model ID from the catalog: ' MAPLE_MODEL
export MAPLE_MODEL
./codex-rs/target/debug/codex \
  -c 'model_provider="maple"' \
  -c 'features.api_key_model_discovery=true' \
  -c 'approval_policy="on-request"' \
  --sandbox workspace-write --model "$MAPLE_MODEL"
```

Use `/model` in the conversation to select another discovered model. Use the
same provider settings with `codex resume --last` for upstream session resume.
These flows require native end-to-end verification. Non-secret persistent
settings are in [examples/maple.config.toml](examples/maple.config.toml).

## Validate real inference and the coding workflow

The optional Python protocol probe uses `MAPLE_API_KEY` from the environment:

```bash
python scripts/probe-maple.py models
python scripts/probe-maple.py chat --model "$MAPLE_MODEL"
python scripts/probe-maple.py tools --model "$MAPLE_MODEL"
```

The tools probe executes a real local addition function and returns its result
to the model. It is a protocol check, not a substitute for coding-agent validation.
The native adapter's streaming behavior has deterministic Rust tests.

Run the actual native agent on the intentionally broken Rust fixture:

```bash
python scripts/smoke-agent.py \
  --binary ./codex-rs/target/debug/codex --model "$MAPLE_MODEL"
```

This stages the original fixture in a temporary Git repository, verifies the
failing baseline tests, and asks the agent to repair the source and run the
tests under workspace-write sandboxing. The script checks the file change,
the original tests and manifest, and the passing test result. It retains the
repository for review. Headless execution denies operations that require
approval. Use the interactive CLI to exercise approval prompts.

## Run it as `maple-code`

[scripts/maple-code](scripts/maple-code) is a launcher for a source build.
[scripts/install-from-source.sh](scripts/install-from-source.sh) builds the
workspace, links the launcher into `~/.local/bin`, writes starter settings and
asks for the API key without echoing it (`--release` builds the optimized
binary). It downloads Maple Proxy from Maple's releases when none is installed,
checking the published SHA-256; `--update-proxy` replaces an installed one. To
do the same by hand, link the
launcher onto your `PATH` and copy the example settings:

```bash
ln -s "$PWD/scripts/maple-code" ~/.local/bin/maple-code
mkdir -p ~/.maple-code
cp examples/maple-code.config.toml ~/.maple-code/config.toml
maple-code maple models
maple-code
```

The launcher reads `.env.maple` unless `MAPLE_API_KEY` is already set, starts a
localhost-only Maple Proxy when none is answering at `MAPLE_BASE_URL` (place the
`maple-proxy` binary in `~/.maple-code/bin/` or on `PATH`), and prefers the
release build over the debug build. It keeps its settings in `~/.maple-code`,
so an ordinary `~/.codex` is untouched. `daemon_auto_start` stays off because a
source build has no installed package for the background app server.

The first catalog request after the proxy starts or idles waits for the proxy's
enclave attestation handshake, measured at 7-14 seconds; later requests take
well under a second. An attestation failure surfaces as a proxy 502; see
`~/.maple-code/proxy.log`.

Use a proxy from [Maple's releases](https://github.com/MaplePrivacyLabs/Maple/releases).
The archived `OpenSecretCloud/maple-proxy` builds and the
`ghcr.io/opensecretcloud/maple-proxy` image are no longer updated. An outdated
proxy fails every request with "PCR0 is not approved by the configured trust
policy" once Maple deploys a newer backend; updating the proxy fixes it.

With the Maple provider active the interface calls itself Maple Code: the
session header, the `exec` banner, the input placeholder and approval messages
("You approved maple-code to run ..."). Other providers keep the Codex name.
The binary and its subcommands are still named `codex`.

Resume hints, the exit menu and the `/` command descriptions follow the same
rule, so a Maple session tells you to run `maple-code resume <id>`.

### Tool names

Maple models see each tool under an opaque alias with the readable name in the
description. A model that answers with the readable name, with or without its
namespace, is matched to the advertised tool when exactly one fits. Any other
name becomes a call to `unavailable_tool`, which Codex answers with an
"unsupported call" result so the model can retry instead of ending the turn.

### Images

Tools such as `view_image` return images in a tool result, which Chat
Completions cannot carry. The adapter leaves a note in the tool result and
sends the image in a user message straight after the tool batch. This needs a
model whose Maple catalog entry reports `vision: true`.

### Full-access mode

`maple-code --full-access` (also `maple-code exec --full-access "..."`) runs
without approval prompts and without the sandbox: the model can run any
command, edit any file your user can, and use the network. It is the same
switch as `--dangerously-bypass-approvals-and-sandbox`. Use it only in a
directory and on a machine where that is acceptable, such as a container or a
throwaway VM. Inside a running session, `/permissions` switches modes.

There is no npm package yet. A scoped npm install remains release work, and no
package name is presumed available.
