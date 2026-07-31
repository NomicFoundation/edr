#!/usr/bin/env bash
#
# Record a perf profile of `hardhat test solidity` via profile-uniswap.ts.
#
# Encapsulates four environment workarounds that are each silently wrong rather
# than loud (see README.md section "Environment caveats"):
#
#   1. sudo resets PATH        -> `sudo env PATH="$PATH"`, else perf can't exec node
#   2. sudo sets HOME=/root    -> pass HOME/USER/LOGNAME, else Hardhat uses
#                                 /root/.cache/hardhat-nodejs and rebuilds from scratch
#   3. workload must not be root -> setpriv drops back to the invoking uid/gid, else
#                                 FFI tests fail and the run short-circuits
#   4. the nvm `node` binary does not symbolize on overlayfs -> run a *copy* placed
#                                 on a resolvable mount, and shim `node` on PATH so
#                                 vm.ffi subprocesses symbolize too
#
# Usage:
#   ./record.sh <fuzz-runs> [output-dir] [-- extra args for profile-uniswap.ts]
#
# Example:
#   ./record.sh 1000 /tmp/prof
#   ./record.sh 10   /tmp/prof -- --grep BitMath

set -euo pipefail

FUZZ_RUNS="${1:?usage: record.sh <fuzz-runs> [output-dir] [-- extra args]}"
OUT_DIR="${2:-/tmp/edr-profiling}"
shift || true
shift 2>/dev/null || true
if [[ "${1:-}" == "--" ]]; then shift; fi
EXTRA_ARGS=("$@")

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LABEL="r${FUZZ_RUNS}"
mkdir -p "$OUT_DIR/shim"

# perf's default `cycles` event is unavailable on linuxkit kernels; perf falls
# back to task-clock on its own, so no -e flag is needed.
SAMPLE_HZ="${SAMPLE_HZ:-999}"

# --- workaround 4: a copy of the node binary on a mount whose dentries resolve.
# Files under /workspaces (ext4) and /tmp resolve; certain overlay inodes -- the
# nvm node binary among them -- are recorded by the kernel as "/ (deleted)".
NODE_REAL="$(readlink -f "$(command -v node)")"
NODE_PROF="$OUT_DIR/node-prof"
if [[ ! -x "$NODE_PROF" || "$NODE_REAL" -nt "$NODE_PROF" ]]; then
  echo "==> staging symbolizable node copy at $NODE_PROF"
  cp -f "$NODE_REAL" "$NODE_PROF"
  chmod +x "$NODE_PROF"
fi
# Same copy named `node`, first on PATH, so `npm`/`node` spawned by vm.ffi
# resolve their symbols as well.
cp -f "$NODE_PROF" "$OUT_DIR/shim/node"
chmod +x "$OUT_DIR/shim/node"

UID_N="$(id -u)"
GID_N="$(id -g)"
if [[ "$UID_N" == "0" ]]; then
  echo "error: run this as a normal user, not root (it re-invokes sudo itself)" >&2
  exit 1
fi

DATA="$OUT_DIR/$LABEL.data"
JSON="$OUT_DIR/phases-$LABEL.json"

echo "==> recording at ${SAMPLE_HZ} Hz -> $DATA"
cd "$HERE"

sudo env \
  PATH="$OUT_DIR/shim:$PATH" \
  HOME="$HOME" USER="${USER:-$(id -un)}" LOGNAME="${LOGNAME:-$(id -un)}" \
  perf record -F "$SAMPLE_HZ" -g -o "$DATA" -- \
  setpriv --reuid="$UID_N" --regid="$GID_N" --clear-groups \
  "$NODE_PROF" --perf-basic-prof --import tsx profile-uniswap.ts \
  --fuzz-runs "$FUZZ_RUNS" --label "perf-$LABEL" --json "$JSON" \
  "${EXTRA_ARGS[@]}"

STACKS="$OUT_DIR/stacks-$LABEL.out"
echo "==> resolving symbols -> $STACKS  (this file is large; hundreds of MB)"
sudo perf script -i "$DATA" > "$STACKS"
sudo chown "$UID_N:$GID_N" "$STACKS"

echo
echo "==> component attribution"
python3 "$HERE/analyze.py" components "$STACKS"

echo
echo "next steps:"
echo "  python3 $HERE/analyze.py frames  $STACKS --thread tokio-rt-worker"
echo "  python3 $HERE/analyze.py rate    $STACKS"
echo "  python3 $HERE/analyze.py pattern $STACKS"
echo "  inferno-collapse-perf < $STACKS | rustfilt | inferno-flamegraph > $OUT_DIR/flame-$LABEL.svg"
