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
#     accessible at all. Consumers reading `MineOrdering.Fifo` need to switch
#     to the string literal `"Fifo"`. v2's `--no-const-enum` produced a
#     regular runtime enum for string enums too; v3 dropped that option.
napi build --platform --no-const-enum "$@" -- --locked
tsc
