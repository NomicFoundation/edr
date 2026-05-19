#!/usr/bin/env bash
# Wrapper that runs the leaked-interval-miner reproducer with a hard timeout,
# so a deadlock surfaces as a definite exit code rather than an indefinite hang.
#
# Platform-agnostic: the leak/deadlock mechanism is V8 GC + finalizer + tokio
# block_on, which is identical on Linux, macOS, and Windows (WSL/bash).
# Only the napi build artifact suffix differs by platform.
#
# Usage:
#   js/deadlock-repro/run.sh [N] [--no-interval]
#
# Exit codes:
#   0   clean — no deadlock detected
#   2   deadlock detected via IntervalMiner::Drop timeout warning
#   124 deadlock detected via process hang (process killed by timeout)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

log() { printf '[%s] %s\n' "$(date '+%H:%M:%S')" "$*" >&2; }
die() { log "ERROR: $*"; exit 1; }

TIMEOUT="${REPRO_TIMEOUT:-30s}"
NO_INTERVAL=""
POSITIONAL=()
for arg in "$@"; do
  case "$arg" in
    --no-interval) NO_INTERVAL="--no-interval" ;;
    *) POSITIONAL+=("$arg") ;;
  esac
done
N="${POSITIONAL[0]:-20}"

# Locate the napi build artifact for any platform.
# napi-rs emits one of: edr.darwin-*.node, edr.linux-*.node, edr.win32-*.node
shopt -s nullglob
artifacts=("$REPO_ROOT"/crates/edr_napi/edr.*.node)
if (( ${#artifacts[@]} == 0 )); then
  die "no edr.*.node artifact found — run 'pnpm -C crates/edr_napi build:dev' first"
fi

# Verify `timeout` is available (GNU coreutils; on macOS install via 'brew install coreutils' → gtimeout)
if ! command -v timeout >/dev/null 2>&1; then
  if command -v gtimeout >/dev/null 2>&1; then
    TIMEOUT_CMD=gtimeout
  else
    die "neither 'timeout' nor 'gtimeout' found; on macOS: brew install coreutils"
  fi
else
  TIMEOUT_CMD=timeout
fi

log "Platform: $(uname -s) $(uname -m)"
log "Build artifact: ${artifacts[0]}"
log "Reproducer N=$N, timeout=$TIMEOUT${NO_INTERVAL:+, interval mining OFF (control case)}"
log ""

# Tee combined output to a temp file so we can detect Drop timeout
# warnings even when the process exits 0.
tmpout=$(mktemp)
trap 'rm -f "$tmpout"' EXIT

set +e
"$TIMEOUT_CMD" "$TIMEOUT" \
  node --expose-gc --max-old-space-size=8192 \
  "$SCRIPT_DIR/leaked-interval-miner.mjs" \
  "$N" ${NO_INTERVAL:+--no-interval} 2>&1 | tee "$tmpout"
# PIPESTATUS[0] = exit code of node/timeout; PIPESTATUS[1] = tee (always 0)
code="${PIPESTATUS[0]}"
set -e

# If the Rust Drop timeout warning fired, the deadlock was detected even
# though the process exited 0 (the 5s cap let it continue).
# Report this as exit 2 so callers can distinguish three outcomes:
#   0   → clean, no deadlock
#   2   → deadlock detected via IntervalMiner::Drop timeout
#   124 → deadlock detected via process hang
if [[ "$code" -eq 0 ]] && grep -q "IntervalMiner::Drop timed out" "$tmpout"; then
  code=2
fi

log ""
case "$code" in
  0)
    if [[ -n "$NO_INTERVAL" ]]; then
      log "EXIT 0 — clean exit (control case: interval mining disabled, no deadlock expected)"
    else
      log "EXIT 0 — no deadlock detected. Either:"
      log "  - This Node version has the fix (Node 24.13.1+ Krypton LTS, or Node 25+)"
      log "  - N=$N wasn't enough to trigger; try bumping: $0 50"
      log "  - The deadlock requires more pressure / specific timing"
    fi
    ;;
  2)
    log "EXIT 2 — DEADLOCK detected via IntervalMiner::Drop timeout warning."
    log "         The 5s Drop timeout prevented an indefinite hang."
    log "         Search the output above for '[edr_provider] WARNING' for details."
    ;;
  124)
    log "EXIT 124 — TIMEOUT after $TIMEOUT. Deadlock reproduced (process hung)."
    log "           The node process was hung in V8 finalize."
    ;;
  *)
    log "EXIT $code — unexpected (see output above)."
    ;;
esac

exit "$code"
