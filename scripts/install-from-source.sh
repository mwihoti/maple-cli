#!/usr/bin/env bash
# install-from-source.sh: build this checkout and put `maple-code` on PATH.
#
# Usage: scripts/install-from-source.sh [--release] [--update-proxy]
#        scripts/install-from-source.sh --clean
#
# This is a source install, not a packaged release: it compiles the Rust
# workspace (slow on a first build), links scripts/maple-code into
# ~/.local/bin, creates starter settings in ~/.maple-code and downloads Maple
# Proxy when none is installed (--update-proxy replaces an installed one). It
# never overwrites an existing config or credentials file.
#
# --clean removes intermediate build output and keeps the built program.
# MAPLE_CODE_SKIP_DISK_CHECK=1 skips the free-space check before a first build.
set -euo pipefail

REPO="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/.." && pwd)"
HOME_DIR="${MAPLE_CODE_HOME:-$HOME/.maple-code}"
BIN_DIR="${MAPLE_CODE_BIN_DIR:-$HOME/.local/bin}"
profile=()
[[ " $* " == *" --release "* ]] && profile=(--release)

# --clean frees the intermediate build output and keeps the built programs, so
# maple-code still runs. The next build starts from scratch.
if [[ " $* " == *" --clean "* ]]; then
  for dir in "$REPO"/maplecode-rs/target/*/; do
    [[ -d "$dir/deps" ]] || continue
    rm -rf "$dir/deps" "$dir/build" "$dir/incremental" "$dir/.fingerprint"
    echo "install: cleaned ${dir%/}"
  done
  exit 0
fi

command -v cargo >/dev/null || {
  echo "install: cargo is required (https://rustup.rs)" >&2
  exit 1
}

# A debug build writes about 32 GB under maplecode-rs/target. The release figure is
# a lower bound: a partial release build had written 5 GB. Running out of space part-way crashes the linker.
kind=debug need_gb=35
[[ ${#profile[@]} -gt 0 ]] && kind=release need_gb=8
free_gb=$(($(df -Pk "$REPO" | awk 'NR==2 {print $4}') / 1024 / 1024))
# A checkout that has been built before only recompiles what changed.
if [[ -z "${MAPLE_CODE_SKIP_DISK_CHECK:-}" && ! -x "$REPO/maplecode-rs/target/$kind/maplecode" ]] &&
  ((free_gb < need_gb)); then
  echo "install: only ${free_gb} GB free; a first $kind build needs at least ${need_gb} GB." >&2
  echo "         Free some space, pass --release for a smaller build, or set" >&2
  echo "         MAPLE_CODE_SKIP_DISK_CHECK=1 to build anyway." >&2
  exit 1
fi

# The incremental cache speeds up repeated edits but grows to tens of gigabytes
# for this workspace. Set CARGO_INCREMENTAL=1 to turn it back on.
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
echo "install: building maplecode ($kind) ..."
(cd "$REPO/maplecode-rs" && cargo build "${profile[@]}" --bin maplecode)

mkdir -p "$BIN_DIR" "$HOME_DIR/bin"
ln -sfn "$REPO/scripts/maple-code" "$BIN_DIR/maple-code"
echo "install: linked $BIN_DIR/maple-code"

if [[ ! -e "$HOME_DIR/config.toml" ]]; then
  cp "$REPO/examples/maple-code.config.toml" "$HOME_DIR/config.toml"
  echo "install: wrote $HOME_DIR/config.toml"
fi

# The key is read without echo and stored in a file Git ignores.
if [[ ! -e "$REPO/.env.maple" && -z "${MAPLE_API_KEY:-}" && -t 0 ]]; then
  read -rsp "Maple API key (input hidden, Enter to skip): " key
  echo
  if [[ -n "$key" ]]; then
    (umask 077 && printf "export MAPLE_API_KEY='%s'\nexport MAPLE_BASE_URL='http://127.0.0.1:8080/v1'\n" "$key" >"$REPO/.env.maple")
    echo "install: wrote $REPO/.env.maple (mode 600, ignored by Git)"
  fi
  unset key
fi

# Maple Proxy comes from Maple's own releases. The archived
# OpenSecretCloud/maple-proxy builds stop passing enclave attestation once
# Maple deploys a newer backend, which shows up as a 502 on every request.
PROXY_VERSION="${MAPLE_PROXY_VERSION:-v3.4.9}"
PROXY_BASE="https://github.com/MaplePrivacyLabs/Maple/releases/download/$PROXY_VERSION"
install_proxy() {
  local asset tmp
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) asset=maple-proxy-linux-x86_64.tar.gz ;;
    Linux-aarch64) asset=maple-proxy-linux-aarch64.tar.gz ;;
    Darwin-arm64) asset=maple-proxy-macos-aarch64.tar.gz ;;
    *) echo "install: no Maple Proxy build for $(uname -s) $(uname -m)" >&2; return 1 ;;
  esac
  tmp="$(mktemp -d)"
  # Chained with && because `set -e` is off inside a function called with ||.
  # The download is refused unless it matches the checksum Maple publishes.
  curl -fsSL -o "$tmp/$asset" "$PROXY_BASE/$asset" &&
    curl -fsSL -o "$tmp/sums" "$PROXY_BASE/maple-proxy-release-final.sha256" &&
    (cd "$tmp" && grep " $asset\$" sums >expected && [[ -s expected ]] &&
      if command -v sha256sum >/dev/null; then sha256sum -c expected; else shasum -a 256 -c expected; fi) >/dev/null &&
    tar -xzf "$tmp/$asset" -C "$tmp" maple-proxy &&
    install -m 755 "$tmp/maple-proxy" "$HOME_DIR/bin/maple-proxy" || {
    rm -rf "$tmp"
    return 1
  }
  rm -rf "$tmp"
  echo "install: installed Maple Proxy $PROXY_VERSION to $HOME_DIR/bin/maple-proxy"
}
if [[ " $* " == *" --update-proxy "* ]] || { [[ ! -x "$HOME_DIR/bin/maple-proxy" ]] && ! command -v maple-proxy >/dev/null; }; then
  install_proxy || {
    echo "install: could not install Maple Proxy. Place a binary at $HOME_DIR/bin/maple-proxy," >&2
    echo "         or run Maple Proxy yourself on 127.0.0.1:8080 before starting maple-code." >&2
  }
fi

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "install: add $BIN_DIR to PATH to run maple-code from anywhere" >&2 ;;
esac
echo "install: done. Try: maple-code maple models"
