#!/usr/bin/env bash

set -euo pipefail

# Stage the coverage library alongside the package so it's bundled when
# publishing, and so consumers of `getCoverageLibrary` can read it at runtime.
cp ../../data/contracts/coverage.sol ./coverage.sol

# NAPI build must be done before the TypeScript compilation.
#
# `--no-const-enum`: napi-rs v3 defaults to emitting `const enum` declarations
# in the generated `.d.ts`. const enums can't be imported as values when the
# consumer enables `isolatedModules: true` (Hardhat does), so we opt out.
#
# Subtlety: v3 changed the meaning of `--no-const-enum` from v2.
#   - For numeric enums (most of EDR's): emits a regular runtime `enum`
#     (same as v2, works under `isolatedModules`).
#   - For string enums (`MineOrdering`, `TestStatus`, `CheatcodeErrorCode`):
#     emits a type-only union (`'Fifo' | 'Priority'`) — values aren't
#     accessible at all.
#
# `--runtime-string-enum` (requires @napi-rs/cli >= 3.7.0): opts string enums
# back into regular runtime `enum` declarations, restoring v2's
# `--no-const-enum` behavior so consumers can keep using `MineOrdering.Fifo`
# as a value.
napi build --platform --no-const-enum --runtime-string-enum "$@" -- --locked

# Verify the generated typings are self-consistent (no dangling type
# references): consumers must compile against them without `skipLibCheck`.
tsc -p tsconfig.typings-check.json

tsc
