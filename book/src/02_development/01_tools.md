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

## Solidity tooling

This tool compiles and instruments Solidity source files for use in EDR tests.

Some integration tests require pre-compiled bytecode. To avoid adding `foundry-compilers` as a test dependency (which can interfere with other test crates that use solc during `cargo test --workspace`), we compile contracts ahead of time. It can also instrument source files with coverage probes, producing instrumented `.sol` files used by the `edr_solidity_tests` crate.

By convention, compiled bytecode in EDR is kept in `data/deployed_bytecode/` as hex-encoded `.in` files. Integration tests load them via `include_str!` so they don't need solc at test time.

### Compile with coverage instrumentation

The `--instrument` flag instruments the source code using EDR's standard coverage instrumentation and automatically includes the coverage library (`data/contracts/coverage.sol`):

```bash
cargo run -p edr_tool_solidity -- --instrument \
  data/contracts/test/CoverageTest.sol
```

### Output only the instrumented source

The `--instrument-only` flag instruments the source without compiling it, printing the instrumented Solidity to stdout:

```bash
cargo run -p edr_tool_solidity -- --instrument-only \
  data/contracts/test/CoverageTest.sol \
  > crates/edr_solidity_tests/tests/testdata/default/coverage/InstrumentedCoverageTest.sol
```

### Write bytecodes to disk

Use `-o` to write `<ContractName>.in` files to a directory:

```bash
cargo run -p edr_tool_solidity -- --instrument \
  -o data/deployed_bytecode \
  data/contracts/test/CoverageTest.sol
```

### Compile with explicit imports

Use `-i` to include additional source files needed by imports. For example, `increment.sol` is a pre-instrumented contract that already contains coverage probe calls and imports `coverage.sol` directly:

```bash
cargo run -p edr_tool_solidity -- data/contracts/increment.sol \
  -i data/contracts/coverage.sol
```

### Solidity source files

Solidity contracts used by EDR tests live in `data/contracts/`. The coverage instrumentation library is at `data/contracts/coverage.sol`.

When adding or modifying contracts, re-run the compile tool to regenerate the `.in` files and update the corresponding test code with any new function selectors shown in the tool output.

## JS runner

Please see the [readme](../../../js/benchmark/README.md) for instructions.
