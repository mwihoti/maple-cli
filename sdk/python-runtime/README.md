# Maple Code CLI Runtime for Python SDK

Platform-specific runtime package consumed by the published `openai-maplecode`.

This package is staged during release so the SDK can pin an exact Maple Code CLI
version without checking platform binaries into the repo.

`openai-maplecode-cli-bin` is intentionally wheel-only. Do not build or publish an
sdist for this package.
