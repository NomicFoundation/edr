#!/usr/bin/env bash
# Reproduces the committed scenarios fixture
# (crates/edr_solidity/fixtures/solx_compiler_output_scenarios.json) from
# pinned public inputs and verifies the bytecode round-trips byte-identically.
#
# The committed compiler input scrubs the forge-std source contents, so the
# repo alone cannot recompile the fixture; this script restores them from the
# forge-std v1.14.0 GitHub tag (the npm registry's `forge-std` package is
# stale and stops at 1.1.2) and compiles with the solx release the fixture
# was generated with (0.1.4, per the CBOR `solcx` stamp in its bytecode).
#
# Expected result: every contract's bytecode and deployedBytecode `object`
# is byte-identical, metadata hash included. The `debugInfo` payloads differ
# only because solx embeds the build directory in the DWARF
# (https://github.com/NomicFoundation/solx/issues/594).
#
# Usage: scripts/reproduce-scenarios-fixture.sh
#   SOLX_BIN=/path/to/solx  overrides the binary (e.g. on non-linux-amd64).
set -euo pipefail

REPO_ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
FIXTURES="$REPO_ROOT/crates/edr_solidity/fixtures"
FORGE_STD_TAG="v1.14.0"
SOLX_VERSION="0.1.4"
# Generation-time length of Scenarios.t.sol; the file is append-only, see
# `scenarios_source_is_append_only` in crates/edr_solidity/src/debug_info/dwarf.rs.
FROZEN_PREFIX_LEN=10244

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

echo "Fetching forge-std $FORGE_STD_TAG..."
curl -sSfL "https://github.com/foundry-rs/forge-std/archive/refs/tags/$FORGE_STD_TAG.tar.gz" \
  | tar xz -C "$WORK_DIR"

if [ -z "${SOLX_BIN:-}" ]; then
  SOLX_BIN="$WORK_DIR/solx"
  echo "Fetching solx $SOLX_VERSION..."
  curl -sSfL -o "$SOLX_BIN" \
    "https://github.com/NomicFoundation/solx/releases/download/$SOLX_VERSION/solx-linux-amd64-gnu-v$SOLX_VERSION"
  chmod +x "$SOLX_BIN"
fi

echo "Splicing sources into the committed compiler input..."
python3 - "$FIXTURES" "$WORK_DIR" "$FROZEN_PREFIX_LEN" <<'PY'
import json, sys

fixtures, work_dir, prefix_len = sys.argv[1], sys.argv[2], int(sys.argv[3])
inp = json.load(open(f"{fixtures}/solx_compiler_input_scenarios.json"))
with open(f"{fixtures}/sources/Scenarios.t.sol", "rb") as f:
    scenarios = f.read()[:prefix_len].decode()
for name, entry in inp["sources"].items():
    if name == "project/contracts/Scenarios.t.sol":
        entry["content"] = scenarios
    else:
        # npm/forge-std@1.14.0/src/X.sol -> forge-std-1.14.0/src/X.sol
        rel = name.split("@", 1)[1].split("/", 1)[1]
        with open(f"{work_dir}/forge-std-1.14.0/{rel}") as f:
            entry["content"] = f.read()
json.dump(inp, open(f"{work_dir}/input.json", "w"))
PY

echo "Compiling..."
"$SOLX_BIN" --standard-json < "$WORK_DIR/input.json" > "$WORK_DIR/output.json"

echo "Comparing against the committed fixture..."
python3 - "$FIXTURES" "$WORK_DIR" <<'PY'
import json, sys

fixtures, work_dir = sys.argv[1], sys.argv[2]
src = "project/contracts/Scenarios.t.sol"
repro = json.load(open(f"{work_dir}/output.json"))
errors = [e for e in repro.get("errors", []) if e.get("severity") == "error"]
if errors:
    sys.exit(f"solx reported errors: {[e.get('message') for e in errors]}")
committed = json.load(open(f"{fixtures}/solx_compiler_output_scenarios.json"))

repro_contracts = repro["contracts"][src]
committed_contracts = committed["contracts"][src]
if set(repro_contracts) != set(committed_contracts):
    sys.exit(f"contract sets differ: {set(committed_contracts) ^ set(repro_contracts)}")

object_diffs = []
debug_info_same = 0
for name, c in committed_contracts.items():
    r = repro_contracts[name]
    for field in ("bytecode", "deployedBytecode"):
        if c["evm"][field]["object"] != r["evm"][field]["object"]:
            object_diffs.append(f"{name}.{field}")
        if c["evm"][field].get("debugInfo") == r["evm"][field].get("debugInfo"):
            debug_info_same += 1

if object_diffs:
    sys.exit(f"BYTECODE DIFFERS (round-trip broken): {object_diffs}")
pairs = 2 * len(committed_contracts)
print(f"OK: all {pairs} bytecode objects byte-identical.")
print(
    f"debugInfo identical for {debug_info_same}/{pairs} pairs — differences "
    "are expected (embedded build dir, solx#594)."
)
PY
