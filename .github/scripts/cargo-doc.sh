#!/bin/bash

set -euo pipefail

edr_pkgs=($(
  cargo metadata --format-version 1 --no-deps \
  | jq -r '.packages[].name | select(startswith("edr_"))'
))

foundry_pkgs=($(
  cargo metadata --format-version 1 --no-deps \
  | jq -r --arg path "$PWD/crates/foundry/" \
    '.packages[] | select(.manifest_path | startswith($path)) | .id'
))

# For EDR crates, test that docs build and they don't have warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features "${edr_pkgs[@]/#/--package=}"

# For Foundry crates, only test that docs build and allow linking to private items
RUSTDOCFLAGS="-A warnings" cargo doc --no-deps --all-features --document-private-items "${foundry_pkgs[@]/#/--package=}"
