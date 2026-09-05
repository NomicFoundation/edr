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
# `--js`/`--dts binding.*`: emit the generated binding as `binding.js`/
# `binding.d.ts` so the hand-written `index.js` wrapper can sit at the package
# entry, subclass `EdrContext`, and re-export the rest. See `index.js`.
napi build --platform --no-const-enum --runtime-string-enum --js binding.js --dts binding.d.ts "$@" -- --locked

# Verify the generated typings are self-consistent (no dangling type
# references): consumers must compile against them without `skipLibCheck`.
tsc -p tsconfig.typings-check.json

# Type-check the hand-written `index.js` wrapper against `binding.d.ts`, so a
# `createProvider` signature change the wrapper no longer matches fails here.
tsc -p tsconfig.check.json

tsc
