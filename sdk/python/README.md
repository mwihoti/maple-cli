# Maple Code Python SDK

Build Python applications that start Maple Code threads, run turns, stream progress,
and control workspace access.

## Install

Install the SDK:

```bash
pip install openai-maplecode
```

## Quickstart

The SDK reuses your existing Maple Code authentication when one is already
available:

```python
from openai_maplecode import Maple Code

with Maplecode() as maplecode:
    thread = maplecode.thread_start()
    result = thread.run("Explain this repository in three bullets.")
    print(result.final_response)
```

`thread.run(...)` returns a `TurnResult` containing the final response,
collected items, and token usage.

## Authentication

Existing Maple Code authentication is reused automatically. To start ChatGPT
browser login explicitly:

```python
from openai_maplecode import Maple Code

with Maplecode() as maplecode:
    login = maplecode.login_chatgpt()
    print(login.auth_url)
    print(login.wait().success)
```

For device-code login:

```python
with Maplecode() as maplecode:
    login = maplecode.login_chatgpt_device_code()
    print(login.verification_url, login.user_code)
    login.wait()
```

For API-key login:

```python
with Maplecode() as maplecode:
    maplecode.login_api_key("sk-...")
```

## Built-In Help

Use Python's standard `help(openai_maplecode)`, `help(Maple Code)`, or
`python -m pydoc openai_maplecode` documentation tools.

## Documentation

- [Getting started](https://github.com/openai/codex/blob/main/sdk/python/docs/getting-started.md)
- [API reference](https://github.com/openai/codex/blob/main/sdk/python/docs/api-reference.md)
- [Untrusted external messages](https://github.com/openai/codex/blob/main/sdk/python/docs/api-reference.md#externalmessage)
- [FAQ](https://github.com/openai/codex/blob/main/sdk/python/docs/faq.md)
- [Examples](https://github.com/openai/codex/blob/main/sdk/python/examples/README.md)

The package is licensed under the
[repository Apache License 2.0](https://github.com/openai/codex/blob/main/LICENSE).
