#!/usr/bin/env bash

set -euo pipefail

# NAPI build must be done before the TypeScript compilation
node ../../node_modules/@napi-rs/cli/scripts/index.js build --platform --no-const-enum "$@"
tsc
