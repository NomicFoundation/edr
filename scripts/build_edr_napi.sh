#!/usr/bin/env bash

set -euo pipefail

# Stage the coverage library alongside the package so it's bundled when
# publishing, and so consumers of `getCoverageLibrary` can read it at runtime.
cp ../../data/contracts/coverage.sol ./coverage.sol

# NAPI build must be done before the TypeScript compilation.
#
# napi-rs v3 default emits `const enum` declarations in the generated `.d.ts`
# (v2 emitted regular runtime `enum`s). `--no-const-enum` in v3 downgrades to
# a type-only union which breaks code that uses these enums as values, so we
# stay on the v3 default. const enums work for the existing Hardhat consumers
# because they don't rely on runtime reflection over the enum object.
napi build --platform "$@" -- --locked
tsc
