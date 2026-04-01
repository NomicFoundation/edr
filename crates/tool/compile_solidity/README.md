# compile-solidity

Compiles Solidity source files and outputs their creation bytecodes. Solc version is auto-detected from the source pragma.

Supports optional coverage instrumentation via `--instrument`, which instruments the source using EDR's standard coverage instrumentation and includes the coverage library automatically.

## Usage

```bash
# Compile and print bytecodes to stdout:
cargo run -p edr_tool_compile_solidity -- \
  data/contracts/increment.sol \
  -i data/contracts/coverage.sol

# Write bytecodes to files (<ContractName>.in):
cargo run -p edr_tool_compile_solidity -- \
  -o data/deployed_bytecode \
  data/contracts/increment.sol \
  -i data/contracts/coverage.sol

# Compile with coverage instrumentation:
cargo run -p edr_tool_compile_solidity -- \
  --instrument \
  data/contracts/test/CoverageTest.sol

# Output only the instrumented source (no compilation):
cargo run -p edr_tool_compile_solidity -- \
  --instrument-only \
  data/contracts/test/CoverageTest.sol
```

## Options

| Flag | Description |
| --- | --- |
| `-o, --output-dir <DIR>` | Write `<ContractName>.in` files to DIR. If omitted, print to stdout. |
| `-i, --include <FILE>` | Additional `.sol` files to include (repeatable) |
| `--instrument` | Instrument source with coverage probes before compiling |
| `--instrument-only` | Only instrument the source (no compilation). Prints to stdout. |
| `--version <VER>` | Solidity version for instrumentation (default: `0.8.26`) |
