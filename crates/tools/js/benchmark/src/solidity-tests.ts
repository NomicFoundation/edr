/*
Baseline

Source: https://github.com/NomicFoundation/forge-std/tree/js-benchmark-config

Foundry version: foundryup --commit 0a5b22f07

Commands:

forge test --fuzz-seed 0x1234567890123456789012345678901234567890 --no-match-test "test_ChainBubbleUp()|test_DeriveRememberKey()"
forge test --fuzz-seed 0x1234567890123456789012345678901234567890 --match-contract "StdCheatsTest"
forge test --fuzz-seed 0x1234567890123456789012345678901234567890 --match-contract "StdCheatsForkTest"
forge test --fuzz-seed 0x1234567890123456789012345678901234567890 --match-contract "StdMathTest"
forge test --fuzz-seed 0x1234567890123456789012345678901234567890 --match-contract "StdStorageTest"
forge test --fuzz-seed 0x1234567890123456789012345678901234567890 --match-contract "StdUtilsForkTest"
 */

import fs from "fs";
import path from "path";
import { simpleGit } from "simple-git";
import {
  buildSolidityTestsInput,
  dirName,
  runAllSolidityTests,
} from "@nomicfoundation/edr-helpers";
import {
  FsAccessPermission,
  SuiteResult,
  EdrContext,
  L1_CHAIN_TYPE,
} from "@nomicfoundation/edr";
import { createHardhatRuntimeEnvironment } from "hardhat/hre";
import { solidityTestConfigToSolidityTestRunnerConfigArgs } from "hardhat/internal/builtin-plugins/solidity-test/helpers";

// This is automatically cached in CI
const RPC_CACHE_PATH = "./edr-cache";

// Total run for all test suites in the  `forge-std` repo
const TOTAL_NAME = "Total";
const TOTAL_EXPECTED_RESULTS = 15;

// Map of test suites to benchmark individually to number of samples (how many times to run the test suite)
export const FORGE_STD_SAMPLES = {
  [TOTAL_NAME]: 5,
  StdCheatsTest: 25,
  StdCheatsForkTest: 45,
  StdMathTest: 65,
  StdStorageTest: 5,
  StdUtilsForkTest: 25,
};

const REPO_DIR = "forge-std";
const REPO_URL = "https://github.com/NomicFoundation/forge-std.git";
const BRANCH_NAME = "js-benchmark-config-hh-v3-release";

/// Run Solidity tests in a Hardhat v3 project. Optionally filter paths with grep
export async function runSolidityTests(
  context: EdrContext,
  chainType: string,
  repoPath: string,
  grep?: string
) {
  const { artifacts, testSuiteIds, tracingConfig, solidityTestsConfig } =
    await createSolidityTestsInput(repoPath);

  let ids = testSuiteIds;
  if (grep !== undefined) {
    ids = ids.filter((id) => {
      const fqn = `${id.source}:${id.name}`;
      return fqn.includes(grep);
    });
  }

  const start = performance.now();
  const results = await runAllSolidityTests(
    context,
    chainType,
    artifacts,
    ids,
    tracingConfig,
    solidityTestsConfig
  );
  const elapsed = performance.now() - start;

  if (results.length === 0) {
    throw new Error(`Didn't run any tests for ${repoPath}`);
  }

  results.sort((a, b) => Number(a.durationNs - b.durationNs));
  for (const result of results) {
    console.log(result.id.name, result.durationNs / 1000000n, result.id.source);
    for (const test of result.testResults) {
      // @ts-ignore
      console.log("  ", test.name, test.durationNs / 1000000n, test.kind.runs);
    }
  }

  console.log(`Ran ${results.length} tests for ${repoPath} in ${elapsed}ms`);

  assertNoFailures(results);
}

/// Run Solidity test benchmarks in the `forge-std` at v3 repo
export async function runForgeStdTests(
  context: EdrContext,
  chainType: string,
  resultsPath: string
) {
  const repoPath = await setupForgeStdRepo();
  const { artifacts, testSuiteIds, tracingConfig, solidityTestsConfig } =
    await createSolidityTestsInput(repoPath);

  const allResults = [];
  const runs = new Map<string, number[]>();
  const recordRun = recordTime.bind(null, runs);

  for (const [name, samples] of Object.entries(FORGE_STD_SAMPLES)) {
    for (let i = 0; i < samples; i++) {
      let ids = testSuiteIds;
      if (name !== TOTAL_NAME) {
        ids = ids.filter((id) => id.name === name);
      }
      const start = performance.now();
      const results = await runAllSolidityTests(
        context,
        chainType,
        artifacts,
        ids,
        tracingConfig,
        solidityTestsConfig
      );
      const elapsed = performance.now() - start;

      const expectedResults = name === TOTAL_NAME ? TOTAL_EXPECTED_RESULTS : 1;
      if (results.length !== expectedResults) {
        throw new Error(
          `Expected ${expectedResults} results for ${name}, got ${results.length}`
        );
      }

      assertNoFailures(results);

      // Log to stderr so that it doesn't pollute stdout where we write the results
      console.error(
        `elapsed (s) on run ${i + 1}/${samples} for ${name}: ${displaySec(elapsed)}`
      );

      if (name === TOTAL_NAME) {
        recordRun(TOTAL_NAME, elapsed);
      } else {
        if (results.length !== 1) {
          throw new Error(
            `Expected 1 result for ${name}, got ${results.length}`
          );
        }
        recordRun(results[0].id.name, elapsed);
      }

      // Hold on to all results to prevent GC from interfering with the benchmark
      allResults.push(results);
    }
  }

  const measurements = getMeasurements(runs);

  // Log info to stderr so that it doesn't pollute stdout where we write the results
  console.error("median total elapsed (s)", displaySec(measurements[0].value));
  console.error("saving results to", resultsPath);

  fs.writeFileSync(resultsPath, JSON.stringify(measurements) + "\n");
}

function getMeasurements(runs: Map<string, number[]>) {
  const results: Array<{ name: string; unit: string; value: number }> = [];

  const total = runs.get(TOTAL_NAME)!;
  results.push({ name: TOTAL_NAME, unit: "ms", value: medianMs(total) });
  runs.delete(TOTAL_NAME);

  const testSuiteNames = Array.from(runs.keys());
  testSuiteNames.sort();

  for (const name of testSuiteNames) {
    const value = medianMs(runs.get(name)!);
    results.push({ name, unit: "ms", value });
  }

  return results;
}

function medianMs(values: number[]) {
  if (values.length % 2 === 0) {
    throw new Error("Expected odd number of values");
  }
  values.sort((a, b) => a - b);
  const half = Math.floor(values.length / 2);
  // Round to get rid of decimal milliseconds
  return Math.round(values[half]);
}

function recordTime(
  runs: Map<string, number[]>,
  name: string,
  elapsed: number
) {
  let measurements = runs.get(name);
  if (measurements === undefined) {
    measurements = [];
    runs.set(name, measurements);
  }
  measurements.push(elapsed);
}

function displaySec(delta: number) {
  const sec = delta / 1000;
  return Math.round(sec * 100) / 100;
}

async function setupForgeStdRepo() {
  const repoPath = path.join(dirName(import.meta.url), "..", REPO_DIR);
  // Ensure directory exists
  if (!fs.existsSync(repoPath)) {
    await simpleGit().clone(REPO_URL, repoPath);
  }

  const git = simpleGit(repoPath);
  await git.fetch();
  await git.checkout(BRANCH_NAME);
  await git.pull();

  return repoPath;
}

async function createSolidityTestsInput(repoPath: string) {
  const configPath = path.join(repoPath, "hardhat.config.js");
  const userConfig = (await import(configPath)).default;
  if (userConfig.solidityTest === undefined) {
    throw new Error(`Missing Solidity test config in ${configPath}`);
  }
  const hre = await createHardhatRuntimeEnvironment(
    userConfig,
    {}, // global options
    repoPath
  );

  const { artifacts, testSuiteIds, tracingConfig } =
    await buildSolidityTestsInput(hre);
  const solidityTestsConfig = solidityTestConfigToSolidityTestRunnerConfigArgs(
    L1_CHAIN_TYPE,
    repoPath,
    userConfig.solidityTest,
    /* verbosity */ 0,
    /* observability */ undefined,
    /* testPattern */ undefined
  );
  // Temporary workaround for `testFuzz_AssumeNotPrecompile` in forge-std which assumes no predeploys on mainnet.
  solidityTestsConfig.localPredeploys = undefined;

  solidityTestsConfig.projectRoot = repoPath;
  solidityTestsConfig.rpcCachePath = RPC_CACHE_PATH;
  const rootPermission = {
    path: repoPath,
    access: FsAccessPermission.DangerouslyReadWriteDirectory,
  };
  if (solidityTestsConfig.fsPermissions !== undefined) {
    solidityTestsConfig.fsPermissions.push(rootPermission);
  } else {
    solidityTestsConfig.fsPermissions = [rootPermission];
  }

  return {
    artifacts,
    testSuiteIds,
    solidityTestsConfig,
    tracingConfig,
  };
}

function assertNoFailures(results: SuiteResult[]) {
  const failed = new Set();
  for (const res of results) {
    for (const r of res.testResults) {
      if (r.status !== "Success") {
        failed.add(`${res.id.name} ${r.name} ${r.status} reason:\n${r.reason}`);
      }
    }
  }
  if (failed.size !== 0) {
    console.error(failed);
    throw new Error(`Some tests failed`);
  }
}
