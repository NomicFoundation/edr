#!/usr/bin/env bash

set -e
set -o pipefail

# FIXME: make it work even if run from other directory that project root
cd ./crates/edr_napi
../../scripts/prepublish.sh
cd ../.. 
pnpm changeset version
