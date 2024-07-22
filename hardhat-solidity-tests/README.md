Temporary directory to experiment with a Hardhat plugin that uses EDR's solidity tests feature.

This directory can be removed once the plugin is handed over to the Hardhat team.

## How to use

1. Clone this repo and [the example](https://github.com/NomicFoundation/hardhat-solidity-tests-example)
2. In this repo, go to `crates/edr_napi` and run `pnpm install && pnpm build`.
3. In this directory, run `pnpm build`
4. In `./example`, run `pnpm test`
