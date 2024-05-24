#!/bin/bash

set -e

# Execute cargo hack only for EDR crates
for dir in crates/edr_*/ ; do
    if [ -d "$dir" ]; then
      pushd "$dir"  
      cargo hack check --feature-powerset --no-dev-deps 
      popd
    fi
done

