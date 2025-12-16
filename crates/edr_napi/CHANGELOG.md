# @nomicfoundation/edr

## 0.12.0-next.19

### Patch Changes

- faef065: Added support for EIP-7892 (Blob Parameter Only hardforks)

## 0.12.0-next.18

### Patch Changes

- 25b2b2d: Added unsupported cheatcode errors for unsupported cheatcodes up to and including Foundry 1.5.0
- 7cc0868: Added support for instrumenting Solidity 0.8.31 source code
- 93a1484: Added Osaka hardfork activations so EDR can accurately infer the hardfork from the block timestamp
- c52f1f6: Added basic support for Jovian hardfork (OP stack)

## 0.12.0-next.17

### Patch Changes

- 5e3968d: Changed latest OP stack hardfork to Isthmus
- 5e3968d: Changed latest L1 hardfork to Osaka
- 5e3968d: Fixed default transaction gas limit for post-Osaka hardforks in OP stack and generic chains

## 0.12.0-next.16

### Minor Changes

- dcf09de: Added full support for OP stack Isthmus hardfork

### Patch Changes

- 2c418b7: Added Osaka blob parameters for usage in eth_call, eth_sendTransaction, etc.
- 8794449: Added validity check for the block RLP size cap
- 16ba197: Added an option to `ProviderConfig` to set the transaction gas cap of the mem pool (EIP-7825)

## 0.12.0-next.15

### Minor Changes

- 1e755a3: Backported features and fixes from Foundry 1.3

### Patch Changes

- 33337ea: Added latest EIP-1559 base fee params to Base Mainnet chain config
- aff229f: Added support for Osaka hardfork

## 0.12.0-next.14

### Patch Changes

- abf7434: Fixed unexpected test failure when running in isolate/gas stats mode
- f67363e: Added latest dynamic base fee parameters to Base Mainnet chain config

## 0.12.0-next.10

### Patch Changes

- 8c1f798: Fixed gas calculation for EIP-7702 refunds

## 0.12.0-next.9

### Minor Changes

- 88a23dc: Fix prepublish step so we again publish the arch/os specific packages.

## 0.12.0-next.8

### Minor Changes

- d0a3a41: Added the ability to collect gas reports for mining blocks and `eth_call`
- c54a499: make all parameters of `eth_feeHistory` rpc call & provider method call required
- d0a3a41: Added the ability to collect gas reports for Solidity tests

### Patch Changes

- a2d99ba: Fixed panic due to invalid GasPriceOracle bytecode for Isthmus OP hardfork
- 23e8ad8: Added docs for `runSolidityTests` arguments.
- 0047135: Update list of unsupported cheatcodes with cheatcodes up to Foundry 1.3.6
- b355c15: Fixed panic when executing eth_call transaction with Isthmus OP hardfork

## 0.12.0-next.7

### Minor Changes

- 20b4311:
  - Added the `ContractDecoder` type
  - Added a function `Provider.contractDecoder` to retrieve the provider's `ContractDecoder` instance
  - Changed `EdrContext.createProvider` to receive a `ContractDecoder` instance instead of a `TracingConfigWithBuffers`
    - The `ContractDecoder` can be constructed from a `TracingConfigWithBuffers` using the static method `ContractDecoder.withContracts`
  - Changed `Provider.addCompilationResult` to no longer return a `boolean`. Failures are now signaled by throwing an exception.

### Patch Changes

- ba6bfa0: Added support for Solidity v0.8.30
- d4806e6: Added a `collectStackTraces` option to `SolidityTestRunnerConfigArgs`, specifying what strategy to use for collecting stack traces

## 0.12.0-next.6

### Minor Changes

- 7b5995f: New `ProviderConfig.baseFeeConfig` field available for configuring different values of eip-1559 `maxChangeDenominator` and `elasticityMultiplier` field.

## 0.12.0-next.5

### Patch Changes

- c498e40: Upgraded revm to v27
- ccab079: Fixed wrong stack trace for expectEmit cheatcode
- 381643b: Removed reference to cli flag from ffi cheatcode error message.

## 0.12.0-next.4

### Minor Changes

- 6640dda: Changed the file system permission config interface for Solidity tests, to mitigate EVM sandbox escape through cheatcodes.

## 0.12.0-next.3

### Minor Changes

- 6ea800c: Removed deprecated JSON-RPC methods: `eth_mining`, `net_listening`, `net_peerCount`, `hardhat_addCompilationResult`, `hardhat_intervalMine`, and `hardhat_reset`.
- a5cc346: Added cheatcode error stack trace entry. This fixes stack traces for errors from expect revert cheatcodes and improves stack traces for other cheatcode errors.

## 0.12.0-next.2

### Patch Changes

- bf9b55b: Improved parallelism in test suite execution for better error reporting and improved performance
- 0e0619e: Added support for function-level gas-tracking in Solidity tests.
- 3f822d8: Fixed panic when using the `pauseGasMetering` cheatcode
- 74b1f05: Made the `contract` field in `CallTrace` optional, and added a separate `address` field that is always present. (Breaking change)

## 0.12.0-next.1

### Minor Changes

- ffd2deb: Added value snapshots to Solidity test runner using gas & value cheatcodes

### Patch Changes

- adbba3d: Fixed mining of local blocks for OP

## 0.12.0-next.0

### Minor Changes

- 8396d70: Changed the ChainConfig to include a chain name and allow timestamp-based hardfork activations
- 097b8c3: Changed ProviderConfig members to decouple from Hardhat 2 concepts
- edc20dc: Added code coverage to the provider. It can be configured through the ProviderConfig
- 5e209a1: Added support for asynchronous callbacks for collected code coverage
- ef49c8a: Removed runSolidityTests method and introduced EdrContext::registerSolidityTestRunnerFactory and EdrContext::runSolidityTests functions as multi-chain alternative
- 097b8c3: Removed unused type definitions from API
- 6f0f557: Added instrumenting of source code for statement code coverage measurement
- ae38942: Added the ability to request execution traces for Solidity tests for either all tests or just failing tests
- 289de8a: Changed the instrumentation API to require a coverage library path
- c21ec83: Replaced `Buffer` with `Uint8Array` in Solidity tests interface
- 306a95e: Added an `ObservabilityConfig` to `SolidityTestRunnerConfigArgs` to allow code coverage measurement
- eb928c6: Fixed panic on stack trace generation for receive function with modifier that calls another method. https://github.com/NomicFoundation/edr/issues/894
- 097b8c3: Moved (and renamed) fork-specific configuration options from `ProviderConfig` to `ForkConfig`
- 097b8c3: Replaced all occurences of Buffer with Uint8Array or ArrayBuffer
- 585fe0b: Changed test and test suite execution time from milliseconds to nanoseconds. Correspondingly, the `durationMs` property of `TestResult` and `SuiteResult` was renamed to `durationNs`.

### Patch Changes

- f606fc6: Fixed instrumentation for control flow statements
- c05b49b: Upgraded revm to v24 and alloy to more recent versions
- f1cdbe2: Turned potential panics into JS errors to help with error reporting to Sentry.
- a6864ff: Added support for local pre-deploys for Solidity tests.
- 1b6d123: Fixed a bug causing async functions to throw errors at the callsite
- ab4e20d: Added hardfork activations for Prague
- 3f85d7d: Fixed `gasPriceOracle` predeploy for local blockchains when using Isthmus hardfork
- 3910948: Deprecated `deleteSnapshot`, `deleteSnapshots`, `revertTo`, `revertToAndDelete`, and `snapshot` cheatcodes in favor of `deleteStateSnapshot`, `deleteStateSnapshots`, `revertToState`, `revertToStateAndDelete`, and `snapshotState`
- 007800e: Fixed custom precompiles not being applied in `eth_sendTransaction`. This enables RIP-7212 support in transactions.
- 2ec6415: Added hardfork activations for Base Mainnet and Base Sepolia.
- c63ea3e: Added new `expectRevert` and `expectPartialRevert` cheatcodes.
- 5e209a1: Fixed bug preventing reporting of collected code coverage
- 4bad80f: Fixed the `withdrawalsRoot` field in mined post-Isthmus block headers of local blockchains
- d903f35: Turned panics during stack trace generation due to invalid assumptions into errors.
- 196af17: Add `allowInternalExpectRevert` config option to customize the behavior of the `expectRevert` cheatcode

## 0.12.0-alpha.0

### Minor Changes

- c9cc954: Removed support for pre-Byzantium root field from RPC receipts
- 2a19726: Replaced the Provider constructor with an API for registering and creating chain-specific providers in the EdrContext

## 0.11.0

### Minor Changes

- 66c7052: Replace const enums with non-const enums in \*.d.ts files

## 0.10.0

### Minor Changes

- 1d377bb: Add Prague hardfork to the list of supported `SpecId`s

## 0.9.0

### Minor Changes

- 9353777: Adds the EIP-7685 requestsHash field to Block RPC response type and BlockOptions
- 314790c: Adds EIP-7702 transactions to eth_call, eth_estimateGas, eth_sendTransaction, eth_sendRawTransaction, and debug_traceCall

### Patch Changes

- 90e3b15: Added the InvalidEXTCALLTarget variant to the ExceptionalHalt enum

## 0.8.0

### Minor Changes

- af26624: Improved provider initialization performance by passing build info as buffer which avoids FFI copy overhead

### Patch Changes

- 4ffb4f6: fix: ignore unknown opcodes in source maps
- 5c8c3dd: Fixed crash when loading EDR on Windows without a C Runtime library installed
- 2b9b805: Improved stack trace generation performance by eliminating one-way branching in the bytecode trie

## 0.7.0

### Minor Changes

- d419f36: Add a `stackTrace` getter to the provider's response and remove the old, lower-level `solidityTrace` getter.

## 0.6.5

### Patch Changes

- 9c062ad: fix: don't panic when a precompile errors

## 0.6.4

### Patch Changes

- 14620c9: Fix panic due to remote nodes not returning total difficulty by adding fallback value

## 0.6.3

### Patch Changes

- a1cee4f: fix(tracing): Make sure to correctly 0x-prefix hex encoded strings in custom error message
- 7047e6c: Fixed panic from unknown opcode in stack traces

## 0.6.2

### Patch Changes

- af56289: fix(tracing): Decode unmapped instructions only at the opcode boundary
- debac88: fix(tracing): Use correct subtrace when detecting error error propagation across delegatecall

## 0.6.1

### Patch Changes

- c8fbf3b: Allow forked block time offset to be negative
- bb3808b: Handle optional solc settings better in the tracing engine

## 0.6.0

### Minor Changes

- a417e19: Renamed `json` field in response to `data` and changed its type to `string | object` (specified as `string | any` due to an napi-rs limitation).

### Patch Changes

- 2f1c189: Improved error message and added option to skip unsupported transaction type in `debug_traceTransaction`. Set `__EDR_UNSAFE_SKIP_UNSUPPORTED_TRANSACTION_TYPES=true` as an environment variable to enable this.

## 0.5.2

### Patch Changes

- 66ca796: fix: added `json` alias property in provider response to fix breaking change in v0.5.1

## 0.5.1

### Patch Changes

- 0bba027: Fixed a bug in fork mode where the locally overridden chain id was used instead of the remote chain id when simulating transactions in pre-fork blocks.
- e02eb22: Fixed a bug in the JSON-RPC where we previously allowed a null value for storage keys of an access list
- 8ae31b9: Fixed crash when returning large JSON responses

## 0.5.0

### Minor Changes

- 07c7667: Added support for RIP-7212 by enabling it in the provider configuration

## 0.4.2

### Patch Changes

- b056511: Fixes restriction to disallow remote blocks without a nonce
- e650c9e: Upgraded revm to v37

## 0.4.1

### Patch Changes

- 245fd07: Provide a more helpful message when passing a timestamp bigger than 2^64 via JSON-RPC
- d87a8c4: Fixed a problem that prevented forking chains without mix hash fields in their blocks
- 5984f11: Fixed missing remote contract code when setting storage

## 0.4.0

### Minor Changes

- 5b4acdd: Added verbose tracing for hardhat-tracer. Breaking change: The `stack_top` property of `edr_napi::trace::TracingStep` was removed and `stack` was added instead. Please see the documentation of the struct for details.

### Patch Changes

- af8053f: Fixed order of parameters for eth_sign (#455)
- 32b765d: Added basic support for `eth_maxPriorityFeePerGas`, with a hardcoded response of 1 gwei (#369)

## 0.3.8

### Patch Changes

- a4d5b62: Added support for blob transactions (EIP-4844) when auto-mining is enabled and the mempool is empty
- ad1a4e8: Added support for EIP-712 payloads with signed integers
- b03c944: Upgraded revm to commit aceb093
- 3dfd1a5: Fixed `smock.fake` causing a panic
- 0db9db9: Added retry for sporadic "missing trie node" JSON-RPC errors
- c5da529: Fixed eth_call and eth_sendTransaction to allow passing input data with both `input` and `data` fields
- caa6f85: Added a workaround for an npm bug by using all builds as normal dependencies

## 0.3.7

_This version didn't include any changes, but it fixed a release issue that caused one of our supported architectures to not be published._

## 0.3.6

### Patch Changes

- e82246c: Removed usage of batch calls to prevent 429 errors
- 7745f35: Fixed mining of blocks when forking a chain that has non-Ethereum block headers
- 8928b77: Improved performance by avoiding call to ecrecover for known sender
- 1a83206: Improved performance - especially for large test suites - by changing internal data structures to achieve constant time checkpointing
- 7745f35: Fixed panic when re-setting global default subscriber
- 9be7023: Improved performance by avoiding clones of transaction traces

## 0.3.5

### Patch Changes

- e5f048e: Removed i686 & ARM builds of EDR
- 3d7f13e: Fixed a bug in `hardhat_metadata` where the local chain id was being used in the fork metadata
- bbc3a6d: Fixed missing KZG point evaluation precompile for Cancun hardfork
- 219f457: Fixed incorrect derivation of hardfork when executing a call in fork mode for the block immediately preceding the fork

## 0.3.4

### Patch Changes

- 71287a8: Removed API keys from RPC error messages
- bdf3971: Fixed 429 HTTP error by using smaller batches when querying genesis account info
- 7c23825: Fixed 429 HTTP error by increasing rate limiting retries
- 92693fb: Fixed calculation of used blob gas for post-Cancun blocks
- 62e28ad: Fixed eth_getLogs RPC request for pre-Merge hardforks

## 0.3.3

### Patch Changes

- 60b2a62: Fixed missing support for hex string as salt in `eth_signTypedData_v4`
- 3ac838b: Fixed detection of Cancun blocks on mainnet
- 7d0f981: Fixed node.js runtime freezing on shutdown

## 0.3.2

### Patch Changes

- b13f58a: Fixed failure when retrieving remote code during state modifications (#5000)
- 19eeeb9: Simplified internal set_account_storage_slot API (#5001)

## 0.3.1

### Patch Changes

- 591b7c5: Fixed failing RPC requests for certain providers due to missing content-type header (#4992)

## 0.3.0

### Minor Changes

- ac155d6: Bump Rust to v1.76

### Patch Changes

- 87da82b: Fixed a problem when forking networks with non-standard transaction types (#4963)
- 5fe1728: Fixed a bug in `hardhat_setStorageAt` that occured when the storage of a remote contract was modified during forking (#4970)
- 0ec305f: Fixed a bug in `hardhat_dropTransaction` where empty queues persisted and caused panics
- a5071e5: Fixed a bug in `eth_estimateGas` where a call would fail because the nonce was being checked
