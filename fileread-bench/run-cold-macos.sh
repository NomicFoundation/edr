#!/usr/bin/env bash
# COLD-cache Rust vs Node file-read benchmark for macOS.
#
# Only the FIRST read of a file after a cache purge is cold. So unlike run.sh
# (which loops with a warm cache), this runs each strategy exactly once with
# its own freshly-purged cache. Numbers are single-shot and therefore noisier;
# run the whole script a few times and compare.
#
# Requires macOS `purge` (ships with the Xcode Command Line Tools) and sudo.
#
# Usage: ./run-cold-macos.sh [numFiles] [sizeBytes]
set -euo pipefail
cd "$(dirname "$0")"

NUM=${1:-4000}
SIZE=${2:-32768}
DATA="./data"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "!! This script is for macOS (Darwin). On Linux use: sync; echo 3 | sudo tee /proc/sys/vm/drop_caches" >&2
fi

ncpu() { sysctl -n hw.logicalcpu 2>/dev/null || echo 4; }

purge_cache() {
  sync
  if command -v purge >/dev/null 2>&1; then
    sudo purge
  else
    echo "!! 'purge' not found — install Xcode CLT (xcode-select --install). Reads will be WARM, not cold." >&2
  fi
}

echo ">> generating $NUM files of $SIZE bytes ($(( NUM * SIZE / 1024 / 1024 )) MiB)"
node generate.mjs "$DATA" "$NUM" "$SIZE"

echo ">> building rust (release)"
(cd rust && cargo build --release -q)

# Cache sudo credentials up front so per-strategy purges don't each prompt.
if command -v purge >/dev/null 2>&1; then
  echo ">> caching sudo credentials for 'purge' (you may be prompted once)"
  sudo -v
fi

BIN=./rust/target/release/fileread-bench

run_cold() { # $1 = label; rest = command to run once (cold)
  local label="$1"; shift
  purge_cache
  echo ""
  echo "#### COLD: $label ####"
  "$@"
}

# Rust: concurrent read wins big on cold cache because many disk ops overlap.
run_cold "Rust threaded (nproc)"        "$BIN" "$DATA" 1 tN
run_cold "Rust sequential"              "$BIN" "$DATA" 1 seq

# Node: this is where the libuv 4-worker cap is expected to matter — raising
# the pool lets more cold disk reads be in flight at once.
run_cold "Node Promise.all pool=4"      node ts/bench.mjs "$DATA" 1 promiseall
run_cold "Node Promise.all pool=$(ncpu)" env UV_THREADPOOL_SIZE="$(ncpu)" node ts/bench.mjs "$DATA" 1 promiseall
run_cold "Node Promise.all pool=64"     env UV_THREADPOOL_SIZE=64 node ts/bench.mjs "$DATA" 1 promiseall
run_cold "Node sequential"              node ts/bench.mjs "$DATA" 1 seq

echo ""
echo ">> done. Re-run a few times; single-shot cold numbers vary."
