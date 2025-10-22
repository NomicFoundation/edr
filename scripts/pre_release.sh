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

# Run prepublish to update platform-specific `package.json` files
cd ./crates/edr_napi
# Run napi prepublish so that platform-specific packages are updated
# to the same version that edr_napi.
# Also to update edr_napi/package.json accordingly
pnpm napi prepublish -t npm --skip-gh-release
# Ignore changes done to napi root package.json (optionalDependencies)
git checkout -- ./package.json
cd ../.. 
