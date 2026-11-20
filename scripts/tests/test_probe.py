import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import unittest
from unittest.mock import patch
import urllib.error

SCRIPT = Path(__file__).resolve().parents[1] / "probe-maple.py"
spec = importlib.util.spec_from_file_location("probe", SCRIPT)
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class ProbeTests(unittest.TestCase):
    def test_rejects_unsafe_endpoint_without_echoing_credential(self):
        for url in ["http://example.com/v1", "https://user:fixture-key@example.com/v1",
                    "https://example.com/v1?key=fixture-key", "https://example.com/v1#fixture-key"]:
            with self.subTest(url=url), patch.dict(os.environ, {"MAPLE_BASE_URL": url}):
                with self.assertRaises(probe.ProbeError) as caught:
                    probe.endpoint()
                self.assertNotIn("fixture-key", str(caught.exception))

    def test_loopback_and_https_supported(self):
        for url in ["http://127.0.0.1:8080/v1", "http://[::1]:8080/v1", "https://proxy.example/v1"]:
            with patch.dict(os.environ, {"MAPLE_BASE_URL": url}):
                self.assertEqual(probe.endpoint(), url)

    def test_missing_key_fails_before_network(self):
        with patch.dict(os.environ, {"MAPLE_API_KEY": ""}), patch.object(probe.urllib.request, "build_opener") as opener:
            with self.assertRaises(probe.ProbeError):
                probe.call("/models")
            opener.assert_not_called()

    def test_auth_errors_do_not_echo_provider_body_or_key(self):
        with patch.dict(os.environ, {"MAPLE_API_KEY": "fixture-key"}), patch.object(probe.urllib.request, "build_opener") as opener:
            response = io.BytesIO(b"Authorization: Bearer fixture-key")
            opener.return_value.open.side_effect = urllib.error.HTTPError(
                "http://localhost/v1/models", 401, "fixture-key", {}, response)
            with self.assertRaises(probe.ProbeError) as caught:
                probe.call("/models")
            self.assertEqual(str(caught.exception), "Authentication rejected")
            self.assertEqual(response.tell(), 0)

    def test_bearer_header_is_attached_to_configured_endpoint_only(self):
        with patch.dict(os.environ, {"MAPLE_API_KEY": "fixture-key", "MAPLE_BASE_URL": "http://127.0.0.1:8080/v1"}), patch.object(probe.urllib.request, "build_opener") as opener:
            opener.return_value.open.return_value = io.BytesIO(b'{"data":[]}')
            self.assertEqual(probe.call("/models"), {"data": []})
            request = opener.return_value.open.call_args.args[0]
            self.assertEqual(request.full_url, "http://127.0.0.1:8080/v1/models")
            self.assertEqual(request.headers["Authorization"], "Bearer fixture-key")

    def test_redirects_are_rejected(self):
        with self.assertRaises(probe.ProbeError):
            probe.RejectRedirect().redirect_request(None, None, 302, "redirect", {}, "https://elsewhere.example")

    def test_tool_probe_performs_addition_and_returns_correct_identifier(self):
        catalog = {"data": [{"id": "fixture-model", "capabilities": {"tool_use": True}}]}
        first = {"choices": [{"finish_reason": "tool_calls", "message": {"role": "assistant", "content": None,
            "tool_calls": [{"id": "fixture-call", "type": "function", "function": {
                "name": "verify_add", "arguments": '{"a":2,"b":3}'}}]}}]}
        final = {"choices": [{"finish_reason": "stop", "message": {"role": "assistant", "content": "The result is 5"}}]}
        with patch("sys.argv", [str(SCRIPT), "tools", "--model", "fixture-model"]), patch.object(probe, "call", side_effect=[catalog, first, final]) as call, contextlib.redirect_stdout(io.StringIO()) as output:
            probe.main()
            messages = call.call_args_list[-1].args[1]["messages"]
            self.assertEqual(messages[-1]["tool_call_id"], "fixture-call")
            self.assertEqual(json.loads(messages[-1]["content"]), {"sum": 5})
            self.assertIn("coding agent still needs smoke validation", output.getvalue())


if __name__ == "__main__":
    unittest.main()
