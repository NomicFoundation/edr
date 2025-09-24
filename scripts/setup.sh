#!/bin/bash
set -euo pipefail

rust_version=$(grep '^channel = ' rust-toolchain | sed 's/.*"\(.*\)".*/\1/')

# rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain $rust_version

# Make rustup available to this script
source "$HOME/.cargo/env"

# Install nightly rustfmt
rustup toolchain install nightly --profile minimal --component rustfmt

sudo apt update

# TODO: nodejs, npm, yarn
# libudev-dev is required by hardhat-ledger
# pkg-config is required by EDR to use OpenSSL
sudo apt install -y libudev-dev pkg-config
