#!/usr/bin/env python3
"""Run the real native Maple-enabled Codex against an intentionally broken crate.

This invokes real inference with workspace-write sandboxing and the native
headless approval policy, which denies operations that require approval.
"""
import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--model", required=True)
    args = parser.parse_args()
    binary = args.binary.expanduser().resolve()
    if not binary.is_file():
        parser.error("build the native Maple-enabled Codex binary first")
    root = Path(__file__).resolve().parents[1]
    workspace = Path(tempfile.mkdtemp(prefix="maple-agent-smoke-"))
    shutil.copytree(root / "fixtures/maple-smoke", workspace, dirs_exist_ok=True,
                    ignore=shutil.ignore_patterns("target"))
    subprocess.run(["git", "init"], cwd=workspace, check=True, stdout=subprocess.DEVNULL)
    subprocess.run(["git", "add", "--", "Cargo.toml", "src/lib.rs"],
                   cwd=workspace, check=True)
    before = (workspace / "src/lib.rs").read_bytes()
    manifest = (workspace / "Cargo.toml").read_bytes()
    test_block = before[before.index(b"#[cfg(test)]"):]
    baseline = subprocess.run(["cargo", "test", "--offline"], cwd=workspace,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if baseline.returncode == 0:
        raise SystemExit("Smoke fixture must fail before the agent runs")
    print(f"Real coding task workspace: {workspace}", flush=True)
    subprocess.run([str(binary), "-c", 'model_provider="maple"',
        "-c", "features.api_key_model_discovery=true",
        "-c", "analytics.enabled=false", "--sandbox", "workspace-write", "--model", args.model,
        "--cd", str(workspace), "exec",
        "Inspect this Rust crate. Fix the add function without changing its tests. Run cargo test --offline, inspect the results, and report what you changed."], check=True)
    if before == (workspace / "src/lib.rs").read_bytes():
        raise SystemExit("Agent did not modify the source file")
    after = (workspace / "src/lib.rs").read_bytes()
    if b"#[cfg(test)]" not in after or after[after.index(b"#[cfg(test)]"):] != test_block:
        raise SystemExit("Agent altered the fixture's tests")
    if manifest != (workspace / "Cargo.toml").read_bytes():
        raise SystemExit("Agent altered the fixture's manifest")
    subprocess.run(["cargo", "test", "--offline"], cwd=workspace, check=True)
    print("Real agent changed a file and produced passing tests under workspace-write sandboxing.")
    print(f"Review files and session results in {workspace}; fixture retained for inspection.")


if __name__ == "__main__":
    main()
