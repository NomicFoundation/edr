#!/usr/bin/env bash
#
# Render a perf capture into browser-viewable SVG flamegraphs (inferno).
#
# The SVGs are interactive: click a frame to zoom, Ctrl-F to search.
#
# Usage:
#   ./render.sh <stacks-file> [output-dir] [label]
#
# Example:
#   ./render.sh /tmp/prof/stacks-r1000.out . r1000

set -euo pipefail

STACKS="${1:?usage: render.sh <stacks-file> [output-dir] [label]}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="${2:-$HERE}"
LABEL="${3:-$(basename "$STACKS" .out | sed 's/^stacks-//')}"

mkdir -p "$OUT_DIR"
OUT_DIR="$(cd "$OUT_DIR" && pwd)"

if [[ ! -f "$STACKS" ]]; then
  echo "error: $STACKS not found" >&2
  exit 1
fi

FOLDED="$OUT_DIR/$LABEL.folded"
# The folded intermediate is large (~109 MB for the runs=1000 capture) and
# regenerable, so drop it however we exit.
trap 'rm -f "$FOLDED"' EXIT

echo "==> folding stacks"
# rustfilt demangles Rust v0 symbols; perf only demangles C++.
inferno-collapse-perf < "$STACKS" | rustfilt > "$FOLDED"

echo "==> full flamegraph"
inferno-flamegraph \
  --title "uniswap-v4-core test solidity ($LABEL)" \
  --countname samples \
  < "$FOLDED" > "$OUT_DIR/flamegraph-$LABEL.svg"

# Native-runner-only view: the full graph is dominated by the FFI subprocesses and
# the JS main thread, which buries the EDR internals.
if grep -q 'tokio-rt-worker' "$FOLDED"; then
  echo "==> EDR-native-only flamegraph"
  grep 'tokio-rt-worker' "$FOLDED" \
    | inferno-flamegraph --title "EDR solidity-test runner only ($LABEL)" --countname samples \
    > "$OUT_DIR/flamegraph-$LABEL-edr-only.svg"
fi

echo
echo "wrote:"
# Exact names, not a glob: label "r10" would otherwise also match "r1000".
find "$OUT_DIR" -maxdepth 1 \
  \( -name "flamegraph-$LABEL.svg" -o -name "flamegraph-$LABEL-edr-only.svg" \) \
  -printf '  %-44f %10s bytes\n' | sort
echo
echo "to view (there is no host file:// path -- /workspaces is a volume inside the VM):"
echo "  $HERE/serve.sh $OUT_DIR"
