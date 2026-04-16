#!/usr/bin/env bash

set -euo pipefail

# Ship the coverage library inside the package so consumers can read it at
# runtime, and so it's transparent what gets injected into instrumented
# contracts.
cp ../../data/contracts/coverage.sol ./coverage.sol

# NAPI build must be done before the TypeScript compilation
napi build --platform --no-const-enum --cargo-flags="--locked" "$@"
tsc
