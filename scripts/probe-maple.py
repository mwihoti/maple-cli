#!/usr/bin/env python3
"""Opt-in authenticated Maple protocol probe. No shell/file tools are exposed.

Uses only Python's standard library. API keys come from MAPLE_API_KEY, never
arguments or files. Tool validation computes a real sum and returns it to the
model; it does not pretend to validate the Codex coding agent.
"""
import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request


class ProbeError(Exception):
    pass


class RejectRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ProbeError("Proxy redirected the request; check the configured base URL")


def endpoint():
    value = os.environ.get("MAPLE_BASE_URL", "http://127.0.0.1:8080/v1").rstrip("/")
    parts = urllib.parse.urlsplit(value)
    if (parts.username or parts.password or parts.query or parts.fragment or not parts.hostname
            or not (parts.scheme == "https" or (parts.scheme == "http" and parts.hostname in ("127.0.0.1", "localhost", "::1")))):
        raise ProbeError("MAPLE_BASE_URL must use HTTPS or loopback HTTP without credentials, query or fragment")
    return value


def call(path, body=None):
    key = os.environ.get("MAPLE_API_KEY", "")
    if not key or any(c.isspace() or ord(c) < 32 for c in key):
        raise ProbeError("Set a valid MAPLE_API_KEY in this terminal; the key is not printed")
    request = urllib.request.Request(endpoint() + path,
        data=None if body is None else json.dumps(body).encode(),
        headers={"Authorization": "Bearer " + key, "Content-Type": "application/json"})
    try:
        with urllib.request.build_opener(RejectRedirect()).open(request, timeout=300) as response:
            data = response.read(8 * 1024 * 1024 + 1)
            if len(data) > 8 * 1024 * 1024:
                raise ProbeError("Probe response exceeded 8 MiB")
            return json.loads(data)
    except urllib.error.HTTPError as error:
        messages = {401: "Authentication rejected", 403: "Account/model access or quota rejected",
                    404: "Endpoint or model unavailable", 429: "Rate limit or quota reached"}
        raise ProbeError(messages.get(error.code, "Proxy returned HTTP " + str(error.code))) from None
    except (urllib.error.URLError, TimeoutError, OSError):
        raise ProbeError("Cannot reach Maple Proxy or request timed out") from None
    except (ValueError, UnicodeError):
        raise ProbeError("Proxy returned malformed JSON") from None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["models", "chat", "tools"])
    parser.add_argument("--model", default=os.environ.get("MAPLE_MODEL"))
    args = parser.parse_args()
    catalog = call("/models")
    models = catalog.get("data")
    if not isinstance(models, list):
        raise ProbeError("Proxy catalog lacks data array")
    if args.mode == "models":
        # JSON escapes terminal controls and preserves unknown capability fields.
        print(json.dumps(models, indent=2, ensure_ascii=True))
        return
    if not args.model:
        raise ProbeError("Pass --model with an exact ID from the models probe")
    matches = [m for m in models if m.get("id") == args.model]
    if len(matches) != 1:
        raise ProbeError("Selected model was not uniquely found in the current catalog")
    messages = [{"role": "user", "content": "Reply with a short greeting."}]
    tools = None
    if args.mode == "tools":
        if matches[0].get("capabilities", {}).get("tool_use") is not True:
            raise ProbeError("Catalog does not confirm this model supports tool use")
        messages = [{"role": "user", "content": "Call verify_add with a=2 and b=3, then report its result."}]
        tools = [{"type": "function", "function": {"name": "verify_add",
            "description": "Compute the integer sum of a and b.", "parameters": {
            "type": "object", "properties": {"a": {"type": "integer"}, "b": {"type": "integer"}},
            "required": ["a", "b"], "additionalProperties": False}}}]
    body = {"model": args.model, "messages": messages, "stream": False}
    if tools:
        body.update(tools=tools, tool_choice="auto")
    first = call("/chat/completions", body)
    choice = first["choices"][0]
    message = choice["message"]
    if args.mode == "chat":
        if choice["finish_reason"] != "stop" or not isinstance(message.get("content"), str) or not message["content"].strip():
            raise ProbeError("Model did not return a complete text answer")
        print("Authenticated chat completion succeeded.")
        return
    calls = message.get("tool_calls", [])
    if choice["finish_reason"] != "tool_calls" or not 1 <= len(calls) <= 8:
        raise ProbeError("Model did not return the requested tool call")
    messages.append(message)
    seen = set()
    for tool in calls:
        identifier = tool.get("id")
        if not isinstance(identifier, str) or not identifier or identifier in seen or tool.get("type") != "function":
            raise ProbeError("Invalid or duplicate tool call identifier/type")
        seen.add(identifier)
        if tool["function"]["name"] != "verify_add":
            raise ProbeError("Model requested an unadvertised tool")
        arguments = json.loads(tool["function"]["arguments"])
        if set(arguments) != {"a", "b"} or any(type(arguments[k]) is not int for k in arguments):
            raise ProbeError("Tool arguments did not satisfy the advertised schema")
        if (arguments["a"], arguments["b"]) != (2, 3):
            raise ProbeError("Model used unexpected probe arguments")
        result = arguments["a"] + arguments["b"]
        messages.append({"role": "tool", "tool_call_id": identifier, "content": json.dumps({"sum": result})})
    second = call("/chat/completions", {"model": args.model, "messages": messages, "tools": tools, "stream": False})
    result = second["choices"][0]
    if result["finish_reason"] != "stop" or "5" not in (result["message"].get("content") or ""):
        raise ProbeError("Model did not complete the tool-result round trip")
    print("Authenticated tool-call round trip succeeded (actual addition tool; coding agent still needs smoke validation).")


if __name__ == "__main__":
    try:
        main()
    except ProbeError as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
    except (KeyError, IndexError, TypeError, ValueError, UnicodeError):
        print("Proxy returned an invalid completion or catalog shape", file=sys.stderr)
        sys.exit(1)
