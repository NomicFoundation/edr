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
import { exec } from "child_process";
import { promisify } from "util";
import Papa from "papaparse";

const execAsync = promisify(exec);
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
  SuiteResult,
  EdrContext,
  StandardTestKind,
  FuzzTestKind,
  InvariantTestKind,
} from "@nomicfoundation/edr";
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
  StdCheatsForkTest: 45,
  StdMathTest: 45,
  StdStorageTest: DEFAULT_SAMPLES,
  StdUtilsForkTest: 15,
};

const REPO_DIR = "forge-std";
const REPO_URL = "https://github.com/NomicFoundation/forge-std.git";
const BRANCH_NAME = "js-benchmark-config-hh-v3";

/// Run Solidity tests in a Hardhat v3 project. Optionally filter paths with grep
export async function runSolidityTests(
  context: EdrContext,
  chainType: string,
  repoPath: string,
  grep?: string
): Promise<string> {
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

  assertNoFailures(results);

  return generateCsvResults(results, repoPath, elapsed * 1000000);
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

function generateCsvResults(
  results: SuiteResult[],
  repoPath: string,
  totalElapsed: number
): string {
  const repoName = path.basename(repoPath);
  const csvData: any[] = [];

  // Individual test results
  for (const suiteResult of results) {
    const testSuiteName = suiteResult.id.name;
    const testSuiteSource = suiteResult.id.source;

    for (const testResult of suiteResult.testResults) {
      const testType = getTestType(testResult.kind);
      const outcome = testResult.status.toLowerCase();
      const runs = getTestRuns(testResult.kind);
      csvData.push({
        repo: repoName,
        test_suite_name: testSuiteName,
        test_suite_source: testSuiteSource,
        test_name: testResult.name,
        test_type: testType,
        outcome,
        duration_ns: testResult.durationNs.toString(),
        runs,
        executor: "edr",
      });
    }
  }

  // Test suite totals
  for (const suiteResult of results) {
    const testSuiteName = suiteResult.id.name;
    const testSuiteSource = suiteResult.id.source;
    csvData.push({
      repo: repoName,
      test_suite_name: testSuiteName,
      test_suite_source: testSuiteSource,
      test_name: "",
      test_type: "suite_total",
      outcome: "",
      duration_ns: suiteResult.durationNs.toString(),
      runs: "",
      executor: "edr",
    });
  }

  // Overall total
  csvData.push({
    repo: repoName,
    test_suite_name: "",
    test_suite_source: "",
    test_name: "",
    test_type: "total",
    outcome: "",
    duration_ns: BigInt(Math.round(totalElapsed)).toString(),
    runs: "",
    executor: "edr",
  });

  // Convert to CSV string using papaparse
  return Papa.unparse(csvData);
}

/// Run forge test --json and generate CSV results
export async function runForgeTests(
  repoPath: string,
  forgePath: string
): Promise<string> {
  const forgeCmd = forgePath;

  // Build the project first (not timed)
  await execAsync(`${forgeCmd} build`, {
    cwd: repoPath,
  });

  const start = performance.now();

  // Execute forge test --json
  const { stdout } = await execAsync(`${forgeCmd} test --json`, {
    cwd: repoPath,
    maxBuffer: 1024 * 1024 * 100, // 100MB buffer for large outputs
  });

  // Total time is not exactly the same as for EDR, as it contains process initialization, reading config from disk, checking the build cache, and then piping the results.
  const elapsed = performance.now() - start;

  const testResults = JSON.parse(stdout);

  return generateForgeTestCsvResults(testResults, repoPath, elapsed * 1000000);
}

function generateForgeTestCsvResults(
  testResults: any,
  repoPath: string,
  totalElapsed: number
): string {
  const repoName = path.basename(repoPath);
  const csvData: any[] = [];

  // Individual test results
  for (const [suitePath, suiteData] of Object.entries(testResults)) {
    const testSuiteName = extractTestSuiteName(suitePath);
    const testSuiteSource = extractTestSuiteSource(suitePath);
    const suiteResults = (suiteData as any).test_results;

    for (const [testName, testData] of Object.entries(suiteResults)) {
      const testType = getForgeTestType((testData as any).kind);
      const outcome = (testData as any).status.toLowerCase();
      const runs = getForgeTestRuns((testData as any).kind);
      const duration = parseForgeTestDuration((testData as any).duration);

      csvData.push({
        repo: repoName,
        test_suite_name: testSuiteName,
        test_suite_source: testSuiteSource,
        test_name: testName,
        test_type: testType,
        outcome,
        duration_ns: duration.toString(),
        runs,
        executor: "forge",
      });
    }
  }

  // Test suite totals
  for (const [suitePath, suiteData] of Object.entries(testResults)) {
    const testSuiteName = extractTestSuiteName(suitePath);
    const testSuiteSource = extractTestSuiteSource(suitePath);
    const suiteDuration = parseForgeTestDuration((suiteData as any).duration);

    csvData.push({
      repo: repoName,
      test_suite_name: testSuiteName,
      test_suite_source: testSuiteSource,
      test_name: "",
      test_type: "suite_total",
      outcome: "",
      duration_ns: suiteDuration.toString(),
      runs: "",
      executor: "forge",
    });
  }

  // Overall total
  csvData.push({
    repo: repoName,
    test_suite_name: "",
    test_suite_source: "",
    test_name: "",
    test_type: "total",
    outcome: "",
    duration_ns: BigInt(Math.round(totalElapsed)).toString(),
    runs: "",
    executor: "forge",
  });

  // Convert to CSV string using papaparse
  return Papa.unparse(csvData);
}

function extractTestSuiteName(suitePath: string): string {
  // Extract test suite name from path like "test/fuzz/casting/CastingUint128.t.sol:CastingUint128_Test"
  const parts = suitePath.split(":");
  return parts[parts.length - 1];
}

function extractTestSuiteSource(suitePath: string): string {
  // Extract source file path from path like "test/fuzz/casting/CastingUint128.t.sol:CastingUint128_Test"
  const parts = suitePath.split(":");
  return parts[0];
}

function getForgeTestType(kind: any): string {
  if (kind.Fuzz) {
    return "fuzz";
  } else if (kind.Invariant) {
    return "invariant";
  } else if (kind.Standard || kind.Unit) {
    return "unit";
  } else {
    throw new Error(`Unknown test type: ${kind}`);
  }
}

function getForgeTestRuns(kind: any): string {
  if (kind.Fuzz) {
    return kind.Fuzz.runs?.toString() || "";
  } else if (kind.Invariant) {
    return kind.Invariant.runs?.toString() || "";
  }
  return "";
}

function parseForgeTestDuration(duration: string): bigint {
  // Parse duration like "5ms 287µs 747ns" into nanoseconds
  const parts = duration.split(" ");
  let totalNs = 0n;

  for (const part of parts) {
    if (part.endsWith("ms")) {
      totalNs += BigInt(Math.round(parseFloat(part.slice(0, -2)) * 1000000));
    } else if (part.endsWith("µs")) {
      totalNs += BigInt(Math.round(parseFloat(part.slice(0, -2)) * 1000));
    } else if (part.endsWith("ns")) {
      totalNs += BigInt(Math.round(parseFloat(part.slice(0, -2))));
    }
  }

  return totalNs;
}

function getTestType(
  kind: StandardTestKind | FuzzTestKind | InvariantTestKind
): string {
  if ("consumedGas" in kind) {
    return "unit";
  } else if ("runs" in kind && "meanGas" in kind) {
    return "fuzz";
  } else if ("runs" in kind && "calls" in kind) {
    return "invariant";
  }
  return "unknown";
}

function getTestRuns(
  kind: StandardTestKind | FuzzTestKind | InvariantTestKind
): string {
  if ("runs" in kind) {
    return kind.runs.toString();
  }
  return "";
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
    observability: {},
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
    repoPath,
    userConfig.solidityTest
  );
  solidityTestsConfig.projectRoot = repoPath;
  solidityTestsConfig.rpcCachePath = RPC_CACHE_PATH;
  const rootPermission = {
    path: repoPath,
    access: FsAccessPermission.ReadWrite,
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
