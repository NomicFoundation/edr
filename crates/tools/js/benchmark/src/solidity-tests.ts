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
  SolidityTestRunnerConfigArgs,
  FsAccessPermission,
  type PathPermission,
  type AddressLabel,
  type StorageCachingConfig,
  type CachedChains,
  type CachedEndpoints,
} from "@ignored/edr";
import { hexStringToBytes } from "@nomicfoundation/hardhat-utils/hex";
import { createHardhatRuntimeEnvironment } from "hardhat/hre";
import { SolidityTestUserConfig } from "hardhat/types/config";

// This is automatically cached in CI
const RPC_CACHE_PATH = "./edr-cache";

// Total run for all test suites in the  `forge-std` repo
const TOTAL_NAME = "Total";
const TOTAL_EXPECTED_RESULTS = 15;

const DEFAULT_SAMPLES = 5;

// Map of test suites to benchmark individually to number of samples (how many times to run the test suite)
export const FORGE_STD_SAMPLES = {
  [TOTAL_NAME]: DEFAULT_SAMPLES,
  StdCheatsTest: DEFAULT_SAMPLES,
  StdCheatsForkTest: 15,
  StdMathTest: 9,
  StdStorageTest: DEFAULT_SAMPLES,
  StdUtilsForkTest: 15,
};

const REPO_DIR = "forge-std";
const REPO_URL = "https://github.com/NomicFoundation/forge-std.git";
const BRANCH_NAME = "js-benchmark-config-hh-v3";

export async function setupForgeStdRepo() {
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

/// Run Solidity tests in a Hardhat v3 repo
export async function runSolidityTests(
  repoPath: string,
  samplesPerSuite: Record<string, number>,
  resultsPath: string
) {
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

  const soltestConfig = solidityTestConfigToSolidityTestRunnerConfigArgs(
    repoPath,
    userConfig.solidityTest
  );
  soltestConfig.projectRoot = repoPath;
  soltestConfig.rpcCachePath = RPC_CACHE_PATH;
  const rootPermission = {
    path: repoPath,
    access: FsAccessPermission.ReadWrite,
  };
  if (soltestConfig.fsPermissions !== undefined) {
    soltestConfig.fsPermissions.push(rootPermission);
  } else {
    soltestConfig.fsPermissions = [rootPermission];
  }

  const allResults = [];
  const runs = new Map<string, number[]>();
  const recordRun = recordTime.bind(null, runs);

  for (const [name, samples] of Object.entries(samplesPerSuite)) {
    for (let i = 0; i < samples; i++) {
      let ids = testSuiteIds;
      if (name !== TOTAL_NAME) {
        ids = ids.filter((id) => id.name === name);
      }
      const start = performance.now();
      const results = await runAllSolidityTests(
        artifacts,
        ids,
        tracingConfig,
        soltestConfig
      );
      const elapsed = performance.now() - start;

      const expectedResults = name === TOTAL_NAME ? TOTAL_EXPECTED_RESULTS : 1;
      if (results.length !== expectedResults) {
        throw new Error(
          `Expected ${expectedResults} results for ${name}, got ${results.length}`
        );
      }

      const failed = new Set();
      for (const res of results) {
        for (const r of res.testResults) {
          if (r.status !== "Success") {
            failed.add(
              `${res.id.name} ${r.name} ${r.status} reason:\n${r.reason}`
            );
          }
        }
      }
      if (failed.size !== 0) {
        console.error(failed);
        throw new Error(`Some tests failed`);
      }

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

// From Hardhat repo
function solidityTestConfigToSolidityTestRunnerConfigArgs(
  projectRoot: string,
  config: SolidityTestUserConfig
): SolidityTestRunnerConfigArgs {
  const fsPermissions: PathPermission[] | undefined = [
    config.fsPermissions?.readWrite?.map((p) => ({ access: 0, path: p })) ?? [],
    config.fsPermissions?.read?.map((p) => ({ access: 0, path: p })) ?? [],
    config.fsPermissions?.write?.map((p) => ({ access: 0, path: p })) ?? [],
  ].flat(1);

  const labels: AddressLabel[] | undefined = config.labels?.map(
    ({ address, label }) => ({
      address: hexStringToBuffer(address),
      label,
    })
  );

  let rpcStorageCaching: StorageCachingConfig | undefined;
  if (config.rpcStorageCaching !== undefined) {
    let chains: CachedChains | string[];
    if (Array.isArray(config.rpcStorageCaching.chains)) {
      chains = config.rpcStorageCaching.chains;
    } else {
      const rpcStorageCachingChains: "All" | "None" =
        config.rpcStorageCaching.chains;
      switch (rpcStorageCachingChains) {
        case "All":
          chains = 0;
          break;
        case "None":
          chains = 1;
          break;
      }
    }
    let endpoints: CachedEndpoints | string;
    if (config.rpcStorageCaching.endpoints instanceof RegExp) {
      endpoints = config.rpcStorageCaching.endpoints.source;
    } else {
      const rpcStorageCachingEndpoints: "All" | "Remote" =
        config.rpcStorageCaching.endpoints;
      switch (rpcStorageCachingEndpoints) {
        case "All":
          endpoints = 0;
          break;
        case "Remote":
          endpoints = 1;
          break;
      }
    }
    rpcStorageCaching = {
      chains,
      endpoints,
    };
  }

  const sender: Buffer | undefined =
    config.sender === undefined ? undefined : hexStringToBuffer(config.sender);
  const txOrigin: Buffer | undefined =
    config.txOrigin === undefined
      ? undefined
      : hexStringToBuffer(config.txOrigin);
  const blockCoinbase: Buffer | undefined =
    config.blockCoinbase === undefined
      ? undefined
      : hexStringToBuffer(config.blockCoinbase);

  return {
    projectRoot,
    ...config,
    fsPermissions,
    labels,
    sender,
    txOrigin,
    blockCoinbase,
    rpcStorageCaching,
  };
}

function hexStringToBuffer(hexString: string): Buffer {
  return Buffer.from(hexStringToBytes(hexString));
}
