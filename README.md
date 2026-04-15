# EDR - Ethereum Development Runtime

**EDR**, or **Ethereum Development Runtime** in full, is a library for creating developer tooling on top of the Ethereum Virtual Machine (EVM), such as an EVM debugger or state inspector.

EDR finds its origins in Hardhat Network but incorporates the lessons we have learned over the years to provide high-performance building blocks for EVM tooling. EDR is written in Rust and provides bindings for the Node API (TypeScript), making it accessible to JavaScript and TypeScript developers.

## Features

- **High-performance EVM execution** thanks to [REVM](https://github.com/bluealloy/revm/)
- **Multi-chain protocol support** with built-in providers for Ethereum L1 and OP Stack chains, and an extensible chain type system for custom chains.
- **Full Ethereum JSON-RPC provider** implementation with support for forking remote JSON-RPC endpointslocally simulated chains, and configurable mining modes (auto-mine, interval, and mempool ordering).
- **`console.log` support** for Solidity with source-mapped logging and argument decoding.
- **Solidity stack traces** with source-mapped error reporting for reverts, panics, custom errors, and out-of-gas conditions.
- **Hierarchical call traces** with decoded function names, arguments, and event logs.
- **Step-level debug traces** with program counter, opcode, gas, stack, memory, and storage information.
- **Solidity test runner** with unit, fuzz (property-based), and invariant test execution, including Foundry-compatible cheatcodes, fork-mode testing against live networks, and counterexample shrinking.
- **Source-level code coverage** via Solidity instrumentation.
- **Per-function and per-deployment gas reports** with proxy delegation chain tracking.

## Production Usage

- [Hardhat 3](https://hardhat.org/)
- [Hardhat 2](https://hardhat.org/hardhat2)
