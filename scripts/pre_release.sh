#!/usr/bin/env bash

set -e
set -x
set -o pipefail

if [ ! -e "crates" ]; then
    echo "Error: Run this script from the monorepo root directory"
    exit 1
fi

# changeset version needs to run first
# so platform-specific packages are updated with the current version
pnpm changeset version

cd ./crates/edr_napi
# Run `napi version` to update version in cross-platform packages
pnpm napi version

cd ../.. 
