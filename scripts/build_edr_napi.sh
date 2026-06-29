#!/usr/bin/env bash

set -euo pipefail

# Stage the coverage library alongside the package so it's bundled when
# publishing, and so consumers of `getCoverageLibrary` can read it at runtime.
cp ../../data/contracts/coverage.sol ./coverage.sol

# NAPI build must be done before the TypeScript compilation
napi build --platform --no-const-enum --cargo-flags="--locked" "$@"

# Emit the library declarations (the `.` wrapper + subpath modules) first, so
# the package self-reference (`import ... from ".."`) resolves to the wrapper
# when the tests are type-checked in the second pass. The second pass only
# type-checks (no emit), avoiding re-emitting — and overwriting — the inputs
# produced by the first pass.
tsc --project tsconfig.build.json
tsc --noEmit
