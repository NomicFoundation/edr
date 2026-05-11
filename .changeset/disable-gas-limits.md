---
"@nomicfoundation/edr": minor
---

- Added `LocalConfig` to specify configuration for locally mined blockchains.

- Added `MiningConfig.blockGasLimit` to optionally specify a block gas limit. When omitted, block gas limit enforcement is disabled.

- Added `ProviderConfig.defaultTransactionGasLimit` to specify a default transaction gas limit to use for RPC call and transaction requests when a `gas` value is not specified.

- Added `ProviderConfig.network` to specify whether a fork or locally mined blockchain is used.

- Changed type of `ProviderConfig.transactionGasCap` to also accept `false`. This allows disabling of the transaction gas cap.

- BREAKING CHANGE: Removed `ProviderConfig.blockGasLimit`. Instead:
  - Use `MiningConfig.blockGasLimit` to set the block gas limit used for mining blocks.
  - Use `ProviderConfig.defaultTransactionGasLimit` to set the default transaction gas limit used for RPC call and transaction requests when a `gas` value is not specified.
  - Use `LocalConfig.genesisBlockGasLimit` to set the block gas limit of the genesis block for locally mined blockchains.

- BREAKING CHANGE: Removed `ProviderConfig.fork`. Instead, use `ProviderConfig.network` with a `ForkConfig` object.

- BREAKING CHANGE: Removed `ProviderConfig.initialBlobGas`. Instead, use `LocalConfig.genesisBlobGas` to set the initial blob gas for locally mined blockchains.

- BREAKING CHANGE: Removed `ProviderConfig.initialDate`. Instead, use `LocalConfig.genesisDate` to set the initial date for locally mined blockchains.

- Added `SolidityTestRunnerConfigArgstransactionGasCap` (defaults to the hardfork default).

- BREAKING CHANGE: Renamed `SolidityTestRunnerConfigArgs.enableTxGasLimitCap` to `SolidityTestRunnerConfigArgs.disableTransactionGasCap` (still defaults to `false`, but the meaning is inverted; i.e. the transaction gas cap is enabled by default).
