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

# Add changes to staged area
git add .

cd ./crates/edr_napi
# Run napi prepublish to update platform-specific `package.json` files
pnpm napi prepublish -t npm --skip-gh-release

# Ignore changes done to napi root `package.json`
# The command adds the `optionalDependencies` field
# We don't want to include it because adding it would make the 
# pnpm-lock.yaml differ with package.json - and `pnpm install` does
# not fix the lockfile in this case.
# TODO: NAPI.rs v3 has a skipOptionalPublish option
git restore ./package.json 

cd ../.. 

# Leave all changes unstaged for commit by the Changesets CI action
git restore --staged .
