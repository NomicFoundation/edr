#!/usr/bin/env bash

set -euo pipefail

# NAPI build must be done before the TypeScript compilation
NAPI_CLI=$(node -p "require.resolve('@napi-rs/cli/scripts/index.js')")
node "$NAPI_CLI" build --platform --no-const-enum "$@"
tsc
