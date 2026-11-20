#!/usr/bin/env python3
"""Recover a complete pinned checkout and apply this inspected-source overlay.

Uses normal Git networking; run from a terminal with network access. Never
overwrites an existing destination, pushes, or publishes anything.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SOURCE_FILES = [
    "codex-rs/cli/Cargo.toml", "codex-rs/cli/src/main.rs", "codex-rs/cli/src/maple_cmd.rs",
    "codex-rs/codex-api/Cargo.toml", "codex-rs/codex-api/src/lib.rs",
    "codex-rs/codex-api/src/endpoint/models.rs", "codex-rs/codex-api/src/endpoint/responses.rs",
    "codex-rs/codex-api/src/endpoint/session.rs", "codex-rs/codex-api/src/maple.rs",
    "codex-rs/codex-api/src/maple_tests.rs", "codex-rs/config/src/config_toml.rs",
    "codex-rs/model-provider-info/Cargo.toml", "codex-rs/model-provider-info/src/lib.rs",
    "codex-rs/model-provider-info/src/maple_credentials.rs",
    "codex-rs/model-provider/src/models_endpoint.rs", "codex-rs/model-provider/src/provider.rs",
    "codex-rs/protocol/src/shell_environment.rs",
    "codex-rs/models-manager/src/manager.rs", "codex-rs/models-manager/src/manager_tests.rs",
]


def git(*args, cwd):
    return subprocess.check_output(["git", *args], cwd=cwd, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--skip-baseline", action="store_true",
                        help="explicitly leave upstream baseline build/testing unverified")
    args = parser.parse_args()
    destination = args.destination.expanduser().resolve()
    if destination.exists():
        parser.error("destination already exists; choose a new directory")
    pin = json.loads((ROOT / "upstream-source.json").read_text())
    subprocess.run(["git", "clone", "--depth", "1", "--branch", pin["tag"],
                    pin["repository"], str(destination)], check=True)
    if git("rev-parse", "HEAD", cwd=destination) != pin["commit"]:
        raise RuntimeError("upstream tag no longer resolves to inspected commit; overlay not applied")
    if git("rev-parse", "HEAD^{tree}", cwd=destination) != pin["tree"]:
        raise RuntimeError("upstream source tree differs from inspected tree; overlay not applied")
    subprocess.run(["git", "switch", "-c", "feature/maple-provider"], cwd=destination, check=True)

    # Baseline failures are recorded before applying the reviewable overlay.
    # Logs contain build/test output only; this script never reads API keys.
    results = []
    if not args.skip_baseline:
        for name, command in [
            ("build", ["cargo", "build", "--locked"]),
            ("tests", ["cargo", "test", "--locked", "-p", "codex-api", "-p", "codex-model-provider-info", "-p", "codex-protocol", "-p", "codex-cli"]),
        ]:
            log_path = destination.parent / (destination.name + f"-baseline-{name}.log")
            print(f"Running baseline {name}; log: {log_path}", flush=True)
            with log_path.open("w") as output:
                result = subprocess.run(command, cwd=destination / "codex-rs", stdout=output, stderr=subprocess.STDOUT)
            results.append({"command": command, "exit_code": result.returncode, "log": str(log_path)})
            outcome = "PASSED" if result.returncode == 0 else "FAILED"
            print(f"Baseline {name}: {outcome} (exit {result.returncode}); log: {log_path}", flush=True)
    for path in SOURCE_FILES:
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, target)
    for directory in ["maple-protocol", "fixtures", "scripts", "examples"]:
        shutil.copytree(ROOT / directory, destination / directory, dirs_exist_ok=True,
                        ignore=shutil.ignore_patterns("target", "__pycache__", "*.pyc"))
    for path in ["README.md", "upstream-source.json", "docs/maple-architecture.md", "docs/maple-validation.md"]:
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, target)
    (destination / "maple-baseline-results.json").write_text(json.dumps({
        "skipped": args.skip_baseline, "results": results,
    }, indent=2) + "\n")
    print(f"Complete pinned source plus Maple changes prepared at {destination}")
    print("Baseline results are in maple-baseline-results.json. Maple integration is not yet built or validated.")
    if any(result["exit_code"] != 0 for result in results):
        print("WARNING: baseline build/tests failed. Inspect the logs and resolve those failures before interpreting Maple build results.")
    print("Next: cd " + str(destination / "codex-rs") + " && cargo test -p codex-api -p codex-model-provider-info -p codex-cli")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (subprocess.CalledProcessError, OSError, RuntimeError) as error:
        print(f"Source preparation failed: {type(error).__name__}", file=sys.stderr)
        sys.exit(1)
