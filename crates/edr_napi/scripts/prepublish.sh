#!/usr/bin/env bash

set -e
set -o pipefail

pnpm napi prepublish -t npm --skip-gh-release

# rename optionalDependencies key to dependencies
jq 'with_entries(if .key == "optionalDependencies" then .key = "dependencies" else . end)' package.json | sponge package.json

# remove os, cpu, libc from builds package.json files
for i in npm/*/package.json; do jq 'del(.os, .cpu, .libc)' $i | sponge $i; done
