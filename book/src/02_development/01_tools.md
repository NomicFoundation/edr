# Tools

The `tools` crate contains various utilities useful for development.

## Benchmarking

Run the hardhat test command in a repo 5 times and report the times it took:

```bash
# From the repo root
 cargo run --bin tools benchmark -i 5 /repo/path -t "npx hardhat test"
```

## Compare test run execution times

Create a provider test execution report for the base branch:

```bash
# From packages/hardhat-core in the base branch
pnpm build && pnpm test:provider -- --reporter json | tee base-test-provider-logs.json
```

Create a provider test execution report for the candidate branch:

```bash
# From packages/hardhat-core in the candidate branch
pnpm build && pnpm test:provider -- --reporter json | tee candidate-test-provider-logs.json
```

Generate a comparison report that will list slower tests in the candidate branch:

```bash
# From the repo root
cargo run --bin tools compare-test-runs base-test-provider-logs.json candidate-test-provider-logs.json > comparisions.txt
```

## Scenarios

Scenarios can be used to collect and replay RPC requests which is useful for performance analysis. Only those requests will be collected that can be successfully deserialized.

### Collect scenario

1. Compile `edr_napi` with the `scenarios` feature
2. Set `EDR_SCENARIO_PREFIX` to the desired prefix for the scenario file name.
3. Execute a test suite with the `EDR_SCENARIO_PREFIX` environment variable set and the freshly compiled `edr_napi` version.
4. The scenario file will be written to the current working directory with the desired file name prefix.
5. Optionally, compress the scenario file `gzip -k <SCENARIO_FILE>`. (The `-k` option preserves the original file, omit it if you want it deleted.)

## Rust runner

### Run scenario

```bash
# From the repo root
cargo run --bin tools --release scenario <PATH_TO_SCENARIO_FILE>
```

The scenario runner supports both compressed and uncompressed scenario files.

The reported running time excludes reading the requests from disk and parsing them.

## Compile Solidity

Some EDR integration tests require pre-compiled Solidity bytecode. To avoid adding `foundry-compilers` as a test dependency (which can interfere with other test crates that use solc during `cargo test --workspace`), we compile contracts ahead of time using this tool.

It compiles Solidity source files and outputs their creation bytecodes along with function selectors. The solc version is auto-detected from the source pragma.

By convention, compiled bytecodes in EDR are kept in `data/deployed_bytecode/` as hex-encoded `.in` files. Integration tests load them via `include_str!` so they don't need solc at test time.

### Compile a contract

```bash
cargo run -p edr_tool_compile_solidity -- data/contracts/increment.sol \
  -i data/contracts/coverage.sol
```

Use `-i` to include additional source files needed by imports.

### Compile with coverage instrumentation

The `--instrument` flag instruments the source code using EDR's standard coverage instrumentation and automatically includes the coverage library (`data/contracts/coverage.sol`):

```bash
cargo run -p edr_tool_compile_solidity -- --instrument \
  data/contracts/test/CoverageTest.sol
```

### Write bytecodes to disk

Use `-o` to write `<ContractName>.in` files to a directory:

```bash
cargo run -p edr_tool_compile_solidity -- --instrument \
  -o data/deployed_bytecode \
  data/contracts/test/CoverageTest.sol
```

### Solidity source files

Solidity contracts used by EDR tests live in `data/contracts/`. The coverage instrumentation library is at `data/contracts/coverage.sol`.

When adding or modifying contracts, re-run the compile tool to regenerate the `.in` files and update the corresponding test code with any new function selectors shown in the tool output.

## JS runner

Please see the [readme](../../../js/benchmark/README.md) for instructions.
