#!/usr/bin/env bash
# COLD-cache Rust vs Node file-read benchmark for macOS.
#
# Only the FIRST read of a file after a cache purge is cold, so we can't loop
# inside a single process (iters is pinned to 1). Instead we repeat the whole
# purge -> single-cold-read cycle REPEATS times per strategy and report the
# median/min/max across those single-shot runs, which denoises the result.
#
# Requires macOS `purge` (ships with the Xcode Command Line Tools) and sudo.
#
# Usage: ./run-cold-macos.sh [numFiles] [sizeBytes] [repeats]
set -euo pipefail
cd "$(dirname "$0")"

NUM=${1:-4000}
SIZE=${2:-32768}
REPEATS=${3:-5}
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

# Repeat a single-shot cold read REPEATS times and aggregate.
# $1 = label; rest = command that prints one "median=<ms> ms" line (iters=1).
run_cold() {
  local label="$1"; shift
  local vals=()
  for ((r = 1; r <= REPEATS; r++)); do
    purge_cache
    local out ms
    out="$("$@" 2>/dev/null)"
    # With iters=1 the reader prints a single "median=<ms> ms" line; grab <ms>.
    ms="$(printf '%s\n' "$out" | sed -nE 's/.*median=[[:space:]]*([0-9.]+).*/\1/p' | head -1)"
    vals+=("${ms:-nan}")
  done
  # min / median / max across the REPEATS single-shot values.
  printf '%s\n' "${vals[@]}" | sort -n | awk -v label="$label" -v n="$REPEATS" -v raw="${vals[*]}" '
    { a[NR] = $1 }
    END {
      med = (NR % 2) ? a[int(NR/2) + 1] : (a[NR/2] + a[NR/2 + 1]) / 2
      printf "COLD  %-30s median=%8.1f ms   min=%8.1f ms   max=%8.1f ms   (n=%d: %s)\n", \
             label, med, a[1], a[NR], n, raw
    }'
}

echo ""
echo "==== COLD cache, $NUM files x $SIZE B, repeats=$REPEATS (each is one purge + one read) ===="
echo ""

# Rust: concurrent read wins big on cold cache because many disk ops overlap.
run_cold "Rust threaded (nproc)"         "$BIN" "$DATA" 1 tN
run_cold "Rust sequential"               "$BIN" "$DATA" 1 seq

# Node: this is where the libuv 4-worker cap is expected to matter — raising
# the pool lets more cold disk reads be in flight at once.
run_cold "Node Promise.all pool=4"       node ts/bench.mjs "$DATA" 1 promiseall
run_cold "Node Promise.all pool=$(ncpu)" env UV_THREADPOOL_SIZE="$(ncpu)" node ts/bench.mjs "$DATA" 1 promiseall
run_cold "Node Promise.all pool=64"      env UV_THREADPOOL_SIZE=64 node ts/bench.mjs "$DATA" 1 promiseall
run_cold "Node sequential"               node ts/bench.mjs "$DATA" 1 seq

echo ""
echo ">> done ($REPEATS purges x 6 strategies)."
