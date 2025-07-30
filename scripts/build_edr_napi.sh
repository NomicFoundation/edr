#!/usr/bin/env bash

set -euo pipefail

# NAPI build must be done before the TypeScript compilation
napi build --platform --no-const-enum "$@"
tsc
