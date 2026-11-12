#!/bin/bash

set -e

# Execute cargo hack only for EDR crates
for dir in crates/edr_*/ ; do
    if [ -d "$dir" ]; then
      pushd "$dir" > /dev/null 
      cargo hack check --feature-powerset --exclude-no-default-features --no-dev-deps
      # Test targets need the dev-dependencies, so they get their own pass.
      cargo hack check --tests --feature-powerset --exclude-no-default-features
      popd > /dev/null
    fi
done

