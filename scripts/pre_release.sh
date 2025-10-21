#!/usr/bin/env bash

set -e
set -o pipefail

if [ ! -e "crates" ]; then
    echo "Error: Run this script from the monorepo root directory"
    exit 1
fi

cd ./crates/edr_napi
../../scripts/prepublish.sh
cd ../.. 
pnpm changeset version
