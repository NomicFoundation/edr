#!/usr/bin/env bash
# Rust vs Node file-read benchmark.
# Usage: ./run.sh [numFiles] [sizeBytes] [iters]
set -euo pipefail
cd "$(dirname "$0")"

NUM=${1:-4000}
SIZE=${2:-32768}
ITERS=${3:-7}
DATA="./data"

echo ">> generating $NUM files of $SIZE bytes"
node generate.mjs "$DATA" "$NUM" "$SIZE"

echo ">> building rust (release)"
(cd rust && cargo build --release -q)

# Warm the page cache so we compare the software/dispatch path, not cold disk.
cat "$DATA"/*.txt > /dev/null 2>&1 || true

echo ""
echo "################ RUST ################"
./rust/target/release/fileread-bench "$DATA" "$ITERS"

echo ""
echo "################ NODE (default threadpool = 4) ################"
node ts/bench.mjs "$DATA" "$ITERS"

echo ""
echo "################ NODE (UV_THREADPOOL_SIZE = nproc) ################"
UV_THREADPOOL_SIZE="$(nproc)" node ts/bench.mjs "$DATA" "$ITERS"

echo ""
echo "################ NODE (UV_THREADPOOL_SIZE = 64) ################"
UV_THREADPOOL_SIZE=64 node ts/bench.mjs "$DATA" "$ITERS"
