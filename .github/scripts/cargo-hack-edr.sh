#!/bin/bash

set -e

# Powerset over all feature combinations is `O(2^n_features)` per crate; running
# it on the full workspace (foundry-* and other forks include large feature
# graphs) makes CI prohibitively slow. Restrict to crates we own and want fully
# covered. Non-edr_* crates are still checked under their default + all-features
# combinations elsewhere (check-edr uses --no-default-features, edr-style
# clippy uses --all-features).
for dir in crates/edr_*/ ; do
    if [ -d "$dir" ]; then
      pushd "$dir" > /dev/null
      cargo hack check --feature-powerset --exclude-no-default-features --no-dev-deps
      popd > /dev/null
    fi
done

