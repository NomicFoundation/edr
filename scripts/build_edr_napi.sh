#!/usr/bin/env bash

set -euo pipefail

# NAPI build must be done before the TypeScript compilation
pnpm exec napi build --platform --no-const-enum "$@"
tsc
