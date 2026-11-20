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

There is currently no `maple-code` executable or npm package. The requested
`maple-code login`, `maple-code models` and scoped npm install become release
work only after these validation gates pass. No package name is presumed available.
