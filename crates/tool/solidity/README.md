# solidity

Solidity tooling for EDR development. Compiles Solidity source files, instruments them with coverage probes, or both.

Solc version is auto-detected from the source pragma.

## Usage

```bash
# Compile with coverage instrumentation:
cargo run -p edr_tool_solidity -- \
  --instrument \
  data/contracts/test/CoverageTest.sol

# Write bytecodes to files (<ContractName>.bin):
cargo run -p edr_tool_solidity -- \
  --instrument \
  -o data/deployment_bytecode \
  data/contracts/test/CoverageTest.sol

# Output only the instrumented source (no compilation):
cargo run -p edr_tool_solidity -- \
  --instrument-only \
  data/contracts/test/CoverageTest.sol

# Compile with explicit imports. For example, `Increment.sol` is a manually
# instrumented contract that imports `coverage.sol` directly:
cargo run -p edr_tool_solidity -- \
  data/contracts/test/Increment.sol \
  -i data/contracts/coverage.sol
```

## Options

| Flag | Description |
| --- | --- |
| `-o, --output-dir <DIR>` | Write `<ContractName>.bin` files to DIR. If omitted, print to stdout. |
| `-i, --include <FILE>` | Additional `.sol` files to include (repeatable) |
| `--instrument` | Instrument source with coverage probes before compiling |
| `--instrument-only` | Only instrument the source (no compilation). Prints to stdout. |
| `--version <VER>` | Solidity version for instrumentation (default: `0.8.26`) |
