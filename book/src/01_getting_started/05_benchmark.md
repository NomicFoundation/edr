# Benchmark

We take two approaches to benchmarking EDR:

1. Automated benchmarks using `criterion`
2. Manual benchmarks using repositories of dependants

## Automated

The criterion benches live in `crates/edr_solidity/benches/` and run with `cargo bench -p edr_solidity --bench <name>`.

### `dwarf_decode`

Measures decoding the DWARF that solx embeds in its bytecode, a load-time cost EDR pays once per contract bytecode section when it loads build info. Per corpus it reports a `full_pass` over every decodable blob and a `largest_blob` bench, both as byte throughput.

```bash
cargo bench -p edr_solidity --bench dwarf_decode
```

Without further setup this benchmarks the committed stack-trace scenarios fixture. Its blobs are small (3.6 KB at most) and decode time grows super-linearly with blob size, so a null result there does not transfer to large projects. To cover those, point `EDR_DWARF_BENCH_DIR` at a directory of `<name>.input.json` / `<name>.output.json` solx standard-JSON pairs; each pair is benchmarked as its own corpus:

```bash
EDR_DWARF_BENCH_DIR=/path/to/corpora cargo bench -p edr_solidity --bench dwarf_decode
```

To build a pair from a project, take `settings` from `crates/edr_solidity/fixtures/solx_compiler_input_stack_trace_scenarios.json` (its `outputSelection` requests `evm.{bytecode,deployedBytecode}.debugInfo` and `ast`), inline the project's sources, and run `solx --standard-json < input.json > output.json`. Check the output's `errors` for `severity == "error"`: a failed compile emits `contracts: {}` and the bench measures nothing. Such corpora are not committed because real projects ship sources under `UNLICENSED` SPDX headers.

For what criterion cannot report, per-stage RSS, per-blob timings to fit a scaling exponent, and an order-independent digest of the decoded output for comparing two revisions, there is a driver:

```bash
cargo run --release -p edr_solidity --example dwarf_ab -- input.json output.json [iters]
```

In CI, `.github/workflows/dwarf-decode-benchmark.yml` runs the committed corpus on pull requests that touch the decode path, its dependencies (`Cargo.lock`) or the bench itself. It benchmarks the PR's merge commit and its base back to back in the same job, so machine variance cancels, and fails only when criterion's 95% confidence interval puts the regression above 10%. It is path-filtered, so a PR outside those paths does not run it.

### `contracts_identifier`

Measures building the contract identifier from solc build info. It needs a compiled checkout of forge-std and skips itself when `EDR_FORGE_STD_ARTIFACTS_DIR` is unset; the setup steps are in the bench's doc header.

## Manual

To measure real-world performance, we use a build of [Hardhat](https://github.com/NomicFoundation/hardhat) with EDR in third-party projects. To make a local build of Hardhat available for linking in other packages, run:

```bash
cd packages/hardhat-core &&
pnpm build &&
pnpm link
```

For this example we will use [openzeppelin-contracts](https://github.com/OpenZeppelin/openzeppelin-contracts):

```bash
git clone https://github.com/OpenZeppelin/openzeppelin-contracts.git &&
cd openzeppelin-contracts &&
pnpm install
```

To use your local hardhat build in a third-party project, run:

```bash
pnpm link hardhat
```

To validate that this worked, you can run:

```bash
file node_modules/hardhat
```

The expected output will look similar to this:

```bash
node_modules/hardhat: symbolic link to ../../hardhat/packages/hardhat-core
```

To prevent the benchmark from being tainted by smart contract compilation, we first run:

```bash
npx hardhat compile
```

Finally, to benchmark the third-party project, we time its test suite. For example:

```bash
time npx hardhat test
```

Resulting in output similar to:

```bash
npx hardhat test  68.99s user 9.59s system 130% cpu 1:00.40 total
```
