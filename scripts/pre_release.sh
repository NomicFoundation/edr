#!/usr/bin/env bash

set -e
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
../../scripts/prepublish.sh # Needs to execute from edr_napi directory
cd ../.. 

# Run pnpm to update pnpm.lock file
# this is necessary since the lockfile references the platform-specific packages
# and their versions have changed
pnpm install --prefer-offline 
