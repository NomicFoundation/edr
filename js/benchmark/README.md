# JS Benchmark Runner

This tool allows benchmarking EDR when invoked from NodeJS to make sure that we account for the FFI overhead when making measurements.

The tool supports benchmarks for two different features: one for the EDR JSON-RPC provider and one for the EDR Solidity test runner.

## Provider Benchmarks

The EDR provider benchmarks consist of JSON-RPC request captured during test runs of third-party Hardhat 2 projects. The purpose of this benchmark is to compare the performance characteristics of different EDR versions.

### Run

```shell
pnpm install

pnpm providerBenchmark --benchmark-output provider-report.json
```

This will run all the provider benchmarks, and it will save the measurements to `./provider-report.json` as json.

The CI is set up to run `providerBenchmark` on every commit in `main`, save the measurements and then compare PRs against the latest measurements from `main`. The measurements from `main` are visualized [here.](https://nomic-foundation-automation.github.io/edr-benchmark-results/bench/)

## Solidity Test Benchmarks

The Solidity test benchmarks consist of third-party repos that use Foundry Forge for testing. The Solidity test benchmarks serve two purposes:

- compare the performance characteristics of different EDR versions
- compare the performance of EDR and Forge

### EDR Solidity Test Benchmarks

This was the first iteration of Solidity test benchmarks. It currently runs select test suites from the `forge-std` library. `forge-std` was initially selected as it has the largest coverage of cheatcodes. The benchmark also doubles as integration testing for `forge-std`.

The benchmark measures the wall clock time elapsed when invoking the EDR Solidity test runner from NodeJS. It's important not to rely on the durations reported in the test results for this benchmark, as those durations don't include the FFI overhead.

The benchmark collects measurements from running all the selected test suites and then each test suite in isolation. The rationale for running each test suite in isolation is that some cheatcode test suites take signficantly longer than others and they could mask regressions in cheatcodes with faster test suites.

#### Run

```shell
pnpm install

# Mainnet Alchemy RPC URL
export ALCHEMY_URL = "..."

pnpm soltestsBenchmark --benchmark-output soltest-report.json
```

The CI is set up to run `soltestsBenchmark` on every commit in `main`, save the measurements and then compare PRs against the latest measurements from `main`. The measurements from `main` are visualized [here.](https://nomic-foundation-automation.github.io/edr-benchmark-results/soltests/)

### Compare EDR and Forge

This is the second iteration of the Solidity test benchmarks that focuses on comparing the performance of EDR and Foundry Forge.

EDR is a Rust library exposed to NodeJS that provides Solidity test execution, while Forge is a pure Rust CLI tool that also handles configuration parsing, building and loading artifacts and reporting results. This means that it's difficult to find a comparison metric that is fair to both projects. E.g. if we measured the wall clock time elapsed when invoking the EDR Solidity test runner from NodeJS (as we do for the EDR Solidity test benchmarks), and compared it with the wall clock time of the Forge CLI execution, that'd be unfair to Forge.

For this reason, we've settled on comparing individual test execution times (as reported in the test results) between EDR and Forge. We think is the only metric where we can make direct comparisons between the two projects. Note that comparing test suite execution times would be a mistake as there is indeterminism in test suite execution due to nested parallelism.

#### Run

```shell
pnpm install
# Optional: install CLI CSV viewer
cargo install xan --locked

# Mainnet Alchemy RPC URL
export ALCHEMY_URL = "..."

# Runs EDR and Forge in each supported third-party repo and creates a CSV file where each row is a test or test suite execution
pnpm compareForge --csv-output edr-forge-stable.csv --count 9 --forge-path ~/.foundry/versions/v1.2.3/forge

# Summarizes the outputs of `compareForge` by repo and test suite type.
pnpm -s reportForge --csv-input edr-forge-v1.2.3.csv | xan view
```

`reportForge` will create output like this:

| repo | successful_tests | failed_tests | total_ratio | unit_ratio | fuzz_ratio | invariant_ratio |
| --- | --- | --- | --- | --- | --- | --- |
| forge-std | 177 | 0 | 0.79 | 0.87 | 0.79 | 0 |
| morpho-blue | 145 | 0 | 0.92 | 1.15 | 0.9 | 0.93 |
| prb-math | 314 | 0 | 1.16 | 0.94 | 1.16 | 0 |
| solady | 1557 | 0 | 0.94 | 0.91 | 0.94 | 0.86 |
| v4-core | 598 | 0 | 0.94 | 0.95 | 0.94 | 0 |

The ratio columns contain the ratio of the cumulative execution time of EDR over Forge, so lower is better for EDR. The zero values in the invariant column mean that the repo didn't have invariant tests.

#### Patches

The `pnpm compareForge` script will check out the supported repos to `./repos` and apply patches to them from `./patches`. If you need to update the patch files, you can follow the following procedure:

```shell
# E.g. to edit the patch file for `prb-math`

# Make sure dependencies are up to date
pnpm install

# This will check out the repos to `./repos` and apply the patches
pnpm compareForge --csv-output out.csv --count 1 --forge-path ~/.foundry/versions/stable/forge --repo prb-math

cd ./repos/hardhat/prb-math

# This will show unstaged changes from applying `./patches/prb-math.patch`
git status

# ... make your changes in ./repos/hardhat/prb-math

# Stage the desired changes that you want to include in the new patch file
git add foundry.toml remappings.txt hardhat.config.js

# Update the patch file in the repo
git diff --cached > ../../../patches/prb-math.patch
```

## Help

Please see `pnpm run help` for more commands and flags.
