#!/usr/bin/env bash

set -e

pnpm build:edr

cd integration
for i in *; do
  if [ -d "$i" ]; then
    echo "Running integration test: '$i'"
    cd $i
    pnpm hardhat test
    cd ..
  fi
done
