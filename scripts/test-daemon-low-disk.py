#!/usr/bin/env python3
"""Rerun compiled daemon integration tests with a compact CLI fixture.

No Cargo rebuild, source edits, cache deletion or /tmp quota changes. The
original debug executable is preserved. Run from a normal host terminal:
python scripts/test-daemon-low-disk.py
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepare-only", action="store_true",
                        help="prepare the fixture without running socket/process tests")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    workspace = root / "maplecode-rs"
    debug = workspace / "target/debug"
    original = debug / "maplecode"
    if not original.is_file():
        parser.error("native debug executable missing; build maplecode first")
    candidates = sorted(p for p in (debug / "deps").glob("app_server_daemon-*")
                        if p.is_file() and not p.suffix and os.access(p, os.X_OK))
    if len(candidates) != 1:
        parser.error("expected one compiled app_server_daemon test executable; found " + str(len(candidates)))
    objcopy = shutil.which("objcopy")
    if objcopy is None:
        parser.error("objcopy is required to prepare the separate compact fixture")
    fixture_dir = workspace / "target/maple-test-bin"
    fixture_dir.mkdir(parents=True, exist_ok=True)
    fixture = fixture_dir / "maplecode"
    if fixture.exists() and fixture.samefile(original):
        parser.error("fixture aliases the original executable; choose a separate fixture path")
    if not fixture.exists() or fixture.stat().st_mtime_ns < original.stat().st_mtime_ns:
        subprocess.run([objcopy, "--strip-all", str(original), str(fixture)], check=True)
    print(f"Original debug executable: {original.stat().st_size // (1024 * 1024)} MiB (unchanged)", flush=True)
    print(f"Separate test fixture: {fixture.stat().st_size // (1024 * 1024)} MiB", flush=True)
    if args.prepare_only:
        print("Fixture prepared; daemon tests have not run.")
        return 0
    test_env = os.environ.copy()
    test_env["CARGO_BIN_EXE_maplecode"] = str(fixture)
    print("Running the existing daemon suite with one test at a time.", flush=True)
    return subprocess.run([str(candidates[0]), "--test-threads=1"],
                          cwd=workspace, env=test_env).returncode


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, subprocess.CalledProcessError) as error:
        print("Daemon test setup failed: " + type(error).__name__, file=sys.stderr)
        sys.exit(1)
