---
"@nomicfoundation/edr": minor
---

- Added `LocalConfig` to specify configuration for locally mined blockchains.

- Added `MiningConfig.blockGasLimit` to optionally specify a block gas limit. When omitted, block gas limit enforcement is disabled.

- BREAKING CHANGE: Removed `ProviderConfig.blockGasLimit`. Use `MiningConfig.blockGasLimit` to set the block gas limit used for mining blocks. use `ProviderConfig.defaultTransactionGasLimit` to set the default transaction gas limit used for RPC call and transaction requests when a `gas` value is not specified. Use `LocalConfig.genesisBlockGasLimit` to set the block gas limit of the genesis block for locally mined blockchains.

- BREAKING CHANGE: Changed `ProviderConfig.transactionGasCap` behavior to disable checks when omitted (previously fell through to the hardfork-default cap on Osaka).

- Added `ProviderConfig.network` to specify whether a fork or locally mined blockchain is used.

- BREAKING CHANGE: Removed `ProviderConfig.fork`. Use `ProviderConfig.network` instead.

- Added `ProviderConfig.defaultTransactionGasLimit` to specify a default transaction gas limit to use for RPC call and transaction requests when a `gas` value is not specified.

- Removed `ProviderConfig.

- BREAKING CHANGE: Renamed `SolidityTestRunnerConfigArgs.enableTxGasLimitCap` to `SolidityTestRunnerConfigArgs.disableTransactionGasCap` (still defaults to `false`, but the meaning is inverted; i.e. the transaction gas cap is enabled by default).

- Added `SolidityTestRunnerConfigArgstransactionGasCap` (defaults to the hardfork default).
