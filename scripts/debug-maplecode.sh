#!/bin/bash

# Set "chatgpt.cliExecutable": "/Users/<USERNAME>/code/maplecode/scripts/debug-maplecode.sh" in VSCode settings to always get the 
# latest maplecode-rs binary when debugging Maplecode Extension.


set -euo pipefail

MAPLECODE_RS_DIR=$(realpath "$(dirname "$0")/../maplecode-rs")
(cd "$MAPLECODE_RS_DIR" && cargo run --quiet --bin maplecode -- "$@")