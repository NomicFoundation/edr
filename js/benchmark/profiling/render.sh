#!/usr/bin/env bash
#
# Render a perf capture into browser-viewable flamegraphs.
#
# Produces both formats, because they are good at different things:
#   *.html  0x's interactive page -- JS-aware, collapsible, has a search box and
#           tier filters (optimized / not-optimized / inlined / C++ / regexp).
#           Best for reading the JS side and for navigating a big tree.
#   *.svg   inferno's flamegraph -- also interactive in a browser (click to zoom,
#           Ctrl-F to search). Much smaller, and trivial to post-process or diff.
#
# The 0x path uses `--visualize-only`, which renders from an existing capture. That
# matters: 0x's own recording mode runs the workload as root, which breaks the FFI
# tests and produces a profile of nothing (see README caveats 2 and 3). Rendering
# from a capture made by record.sh avoids that entirely.
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

echo "==> inferno SVG"
# rustfilt demangles Rust v0 symbols; perf only demangles C++.
inferno-collapse-perf < "$STACKS" | rustfilt > "$OUT_DIR/$LABEL.folded"
inferno-flamegraph \
  --title "uniswap-v4-core test solidity ($LABEL)" \
  --countname samples \
  < "$OUT_DIR/$LABEL.folded" > "$OUT_DIR/flamegraph-$LABEL.svg"

# Native-runner-only view: the full graph is dominated by the FFI subprocesses and
# the JS main thread, which buries the EDR internals.
if grep -q 'tokio-rt-worker' "$OUT_DIR/$LABEL.folded"; then
  grep 'tokio-rt-worker' "$OUT_DIR/$LABEL.folded" \
    | inferno-flamegraph --title "EDR solidity-test runner only ($LABEL)" --countname samples \
    > "$OUT_DIR/flamegraph-$LABEL-edr-only.svg"
fi

echo "==> 0x HTML"
# 0x --visualize-only discovers the capture by filename: /^stacks\.(.*)\.out$/.
# A file named stacks-r1000.out will NOT be found, hence the staging copy.
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp "$STACKS" "$STAGE/stacks.1.out"
cat > "$STAGE/meta.json" <<EOF
{"title":"uniswap-v4-core test solidity ($LABEL)","name":"flamegraph"}
EOF

# Large captures (the runs=1000 dump is ~444 MB) need more than the default heap.
( cd "$STAGE" && NODE_OPTIONS="--max-old-space-size=12000" 0x --visualize-only . >/dev/null 2>&1 )
cp "$STAGE/flamegraph.html" "$OUT_DIR/flamegraph-$LABEL.html"

# The folded intermediate is large and regenerable; keep only the renders.
rm -f "$OUT_DIR/$LABEL.folded"

echo
echo "wrote:"
find "$OUT_DIR" -maxdepth 1 -name "*$LABEL*" -printf '  %-44f %10s bytes\n' | sort
echo
echo "to view (there is no host file:// path -- /workspaces is a volume inside the VM):"
echo "  $HERE/serve.sh $OUT_DIR"
