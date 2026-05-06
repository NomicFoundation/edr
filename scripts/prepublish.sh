#!/usr/bin/env bash

set -e
set -x
set -o pipefail

pnpm napi pre-publish -t npm -p npm

jq 'with_entries(if .key == "optionalDependencies" then .key = "dependencies" else . end)' package.json | sponge package.json
