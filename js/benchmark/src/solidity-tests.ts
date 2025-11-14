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
  FsAccessPermission,
  SuiteResult,
  EdrContext,
  StandardTestKind,
  FuzzTestKind,
  InvariantTestKind,
  L1_CHAIN_TYPE,
  l1SolidityTestRunnerFactory,
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

interface RepoData {
  url: string;
  commit: string;
  patchFile?: string;
}

// The external repos are patched with a Hardhat 3 config and to make sure that results are comparable (e.g. by setting fuzz seeds for both HH3 and Foundry or explicitly setting the solc version).
export const REPOS: Record<string, RepoData> = {
  "forge-std": {
    url: "https://github.com/NomicFoundation/forge-std.git",
    commit: "a3dca253700f19f15b1837c57c67b9388f5cc3fb",
    // Some tests for cheatcodes not supported by EDR have been commented out.
    // Tests that write files on disk have been edited for improved reliability.
    patchFile: "forge-std.patch",
  },
  "morpho-blue": {
    url: "https://github.com/morpho-org/morpho-blue.git",
    commit: "8eb9c89d3b24866ce9fef7c1d18b34427e937843",
    // Inline `allow_internal_expect_revert = true` config was replaced by the global one, as HH3 doesn't support inline configuration yet.
    patchFile: "morpho-blue.patch",
  },
  "prb-math": {
    url: "https://github.com/PaulRBerg/prb-math.git",
    commit: "aad73cfc6cdc2c9b660199b5b1e9db391ea48640",
    patchFile: "prb-math.patch",
  },
  solady: {
    url: "https://github.com/Vectorized/solady.git",
    commit: "271807270b1e14e541a231ff76a869accca7546d",
    // Deleted files specified in the `skip` option in foundry.toml as HH3 doesn't support this option.
    // Removed remappings from foundry.toml and created remappings.txt as HH3 only supports the latter.
    patchFile: "solady.patch",
  },
  "uniswap-v4-core": {
    url: "https://github.com/Uniswap/v4-core.git",
    commit: "59d3ecf53afa9264a16bba0e38f4c5d2231f80bc",
    // Global fuzz runs config was reduced to 10 to match the inline config for one test, as HH3 doesn't support inline configuration yet.
    patchFile: "uniswap-v4-core.patch",
  },
};

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

  const start = process.hrtime.bigint();
  const [, results] = await runAllSolidityTests(
    context,
    chainType,
    artifacts,
    ids,
    tracingConfig,
    solidityTestsConfig
  );
  const elapsedNs = process.hrtime.bigint() - start;

  if (results.length === 0) {
    throw new Error(`Didn't run any tests for ${repoPath}`);
  }

  assertNoFailures(results);

  return generateCsvResults(results, repoPath, elapsedNs);
}

/// Run Solidity test benchmarks in the `forge-std` at v3 repo
export async function runSolidityTestsBenchmark(resultsPath: string) {
  const context = new EdrContext();
  const chainType = L1_CHAIN_TYPE;
  await context.registerSolidityTestRunnerFactory(
    chainType,
    l1SolidityTestRunnerFactory()
  );

  const repoPath = await setupRepo(
    REPOS["forge-std"],
    "hardhat",
    // Since this is run in CI, make sure we reset before each run
    /* cleanFirst */ true
  );
  const { artifacts, testSuiteIds, tracingConfig, solidityTestsConfig } =
    await createSolidityTestsInput(repoPath);

  const allResults = [];
  const runs = new Map<string, bigint[]>();
  const recordRun = recordTime.bind(null, runs);

  for (const [name, samples] of Object.entries(FORGE_STD_SAMPLES)) {
    for (let i = 0; i < samples; i++) {
      let ids = testSuiteIds;
      if (name !== TOTAL_NAME) {
        ids = ids.filter((id) => id.name === name);
      }
      const startNs = process.hrtime.bigint();
      const [, results] = await runAllSolidityTests(
        context,
        chainType,
        artifacts,
        ids,
        tracingConfig,
        solidityTestsConfig
      );
      const elapsedNs = process.hrtime.bigint() - startNs;

      const expectedResults = name === TOTAL_NAME ? TOTAL_EXPECTED_RESULTS : 1;
      if (results.length !== expectedResults) {
        throw new Error(
          `Expected ${expectedResults} results for ${name}, got ${results.length}`
        );
      }

      assertNoFailures(results);

      // Log to stderr so that it doesn't pollute stdout where we write the results
      console.error(
        `elapsed (s) on run ${i + 1}/${samples} for ${name}: ${displaySecFromNs(elapsedNs)}`
      );

      if (name === TOTAL_NAME) {
        recordRun(TOTAL_NAME, elapsedNs);
      } else {
        if (results.length !== 1) {
          throw new Error(
            `Expected 1 result for ${name}, got ${results.length}`
          );
        }
        recordRun(results[0].id.name, elapsedNs);
      }

      // Hold on to all results to prevent GC from interfering with the benchmark
      allResults.push(results);
    }
  }

  const measurements = getMeasurements(runs);

  // Log info to stderr so that it doesn't pollute stdout where we write the results
  console.error(
    "median total elapsed (s)",
    displaySecFromUs(measurements[0].value)
  );
  console.error("saving results to", resultsPath);

  fs.writeFileSync(resultsPath, JSON.stringify(measurements) + "\n");
}

function getMeasurements(runs: Map<string, bigint[]>) {
  const results: Array<{ name: string; unit: string; value: number }> = [];

  const totalNs = runs.get(TOTAL_NAME)!;
  results.push({ name: TOTAL_NAME, unit: "us", value: medianUs(totalNs) });
  runs.delete(TOTAL_NAME);

  const testSuiteNames = Array.from(runs.keys());
  testSuiteNames.sort();

  for (const name of testSuiteNames) {
    const value = medianUs(runs.get(name)!);
    results.push({ name, unit: "us", value });
  }

  return results;
}

function generateCsvResults(
  results: SuiteResult[],
  repoPath: string,
  totalElapsedNs: bigint
): string {
  const repoName = path.basename(repoPath);
  const csvData: any[] = [];

  // Individual test results
  for (const suiteResult of results) {
    const testSuiteName = suiteResult.id.name;
    const testSuiteSource = normalizeSuiteResultSource(suiteResult.id.source);

    for (const testResult of suiteResult.testResults) {
      const testType = getTestType(testResult.kind);
      const outcome = testResult.status.toLowerCase();
      const runs = getTestRuns(testResult.kind);
      csvData.push({
        repo: repoName,
        testSuiteName,
        testSuiteSource,
        testName: testResult.name,
        testType,
        outcome,
        durationNs: testResult.durationNs.toString(),
        runs,
        executor: "edr",
      });
    }
  }

  // Test suite totals
  for (const suiteResult of results) {
    const testSuiteName = suiteResult.id.name;
    const testSuiteSource = normalizeSuiteResultSource(suiteResult.id.source);
    csvData.push({
      repo: repoName,
      testSuiteName,
      testSuiteSource,
      testName: "",
      testType: "suite_total",
      outcome: "",
      durationNs: suiteResult.durationNs.toString(),
      runs: "",
      executor: "edr",
    });
  }

  // Overall total
  csvData.push({
    repo: repoName,
    testSuiteName: "",
    testSuiteSource: "",
    testName: "",
    testType: "total",
    outcome: "",
    durationNs: totalElapsedNs.toString(),
    runs: "",
    executor: "edr",
  });

  // Convert to CSV string using papaparse
  return Papa.unparse(csvData);
}

function normalizeSuiteResultSource(source: string): string {
  // Hardhat adds this prefix to source files in the repo
  const HARDHAT_PROJECT_PREFIX = "project/";
  // Hardhat adds this prefix to npm dependencies
  const HARDHAT_NPM_PREFIX = "npm/";

  if (source.startsWith(HARDHAT_PROJECT_PREFIX)) {
    return source.slice(HARDHAT_PROJECT_PREFIX.length);
  } else if (source.startsWith(HARDHAT_NPM_PREFIX)) {
    return source.slice(HARDHAT_NPM_PREFIX.length);
  } else {
    return source;
  }
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

  const start = process.hrtime.bigint();

  // Execute forge test --json
  const { stdout } = await execAsync(`${forgeCmd} test --json`, {
    cwd: repoPath,
    maxBuffer: 1024 * 1024 * 100, // 100MB buffer for large outputs
  });

  // Total time is not exactly the same as for EDR, as it contains process initialization, reading config from disk, checking the build cache, and then piping the results.
  const elapsedNs = process.hrtime.bigint() - start;

  const testResults = JSON.parse(stdout);

  return generateForgeTestCsvResults(testResults, repoPath, elapsedNs);
}

function generateForgeTestCsvResults(
  testResults: any,
  repoPath: string,
  totalElapsedNs: bigint
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
        testSuiteName,
        testSuiteSource,
        testName,
        testType,
        outcome,
        durationNs: duration.toString(),
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
      testSuiteName,
      testSuiteSource,
      testName: "",
      testType: "suite_total",
      outcome: "",
      durationNs: suiteDuration.toString(),
      runs: "",
      executor: "forge",
    });
  }

  // Overall total
  csvData.push({
    repo: repoName,
    testSuiteName: "",
    testSuiteSource: "",
    testName: "",
    testType: "total",
    outcome: "",
    durationNs: totalElapsedNs.toString(),
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

interface ForgeTestKind {
  Fuzz?: { runs: number };
  Invariant?: { runs: number };
  Standard?: {};
  Unit?: {};
}

function getForgeTestType(kind: ForgeTestKind): string {
  if (kind.Fuzz !== undefined) {
    return "fuzz";
  } else if (kind.Invariant !== undefined) {
    return "invariant";
  } else if (kind.Standard !== undefined || kind.Unit !== undefined) {
    return "unit";
  } else {
    throw new Error(`Unknown test type: ${kind}`);
  }
}

function getForgeTestRuns(kind: ForgeTestKind): string {
  if (kind.Fuzz !== undefined) {
    return kind.Fuzz.runs.toString();
  } else if (kind.Invariant !== undefined) {
    return kind.Invariant.runs.toString();
  }
  return "";
}

interface LegacyForgeTestDuration {
  secs: number;
  nanos: number;
}

function isLegacyForgeTestDuration(obj: any): obj is LegacyForgeTestDuration {
  return typeof obj.secs === "number" && typeof obj.nanos === "number";
}

export function parseForgeTestDuration(
  duration: string | LegacyForgeTestDuration
): bigint {
  if (isLegacyForgeTestDuration(duration)) {
    return BigInt(duration.secs) * 1000000000n + BigInt(duration.nanos);
  }

  if (duration.length === 0) {
    throw new Error("Expected duration, got empty string");
  }

  // Parse duration like "5ms 287µs 747ns" into nanoseconds
  const parts = duration.split(" ");
  let totalNs = 0n;

  for (const part of parts) {
    // Use regex to split number and unit exactly
    const match = part.match(/^(\d+)([a-zA-Zµ]+)$/);
    if (match === null) {
      throw new Error(`Invalid duration format: ${part}`);
    }

    const [, numberStr, unit] = match;
    const value = parseInt(numberStr, 10);
    if (value >= 1000) {
      throw new Error(`Expected value to be less than 1000, got '${value}'`);
    }

    // Exact unit matching
    switch (unit) {
      case "ns":
        totalNs += BigInt(value);
        break;
      case "µs":
        totalNs += BigInt(value) * 1_000n;
        break;
      case "us":
        totalNs += BigInt(value) * 1_000n;
        break;
      case "ms":
        totalNs += BigInt(value) * 1_000_000n;
        break;
      case "s":
        totalNs += BigInt(value) * 1_000_000_000n;
        break;
      case "m":
        totalNs += BigInt(value) * 60n * 1_000_000_000n;
        break;
      case "h":
        totalNs += BigInt(value) * 60n * 60n * 1_000_000_000n;
        break;
      default:
        throw new Error(`Unknown duration unit: ${unit}`);
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

function medianUs(valuesNs: bigint[]) {
  if (valuesNs.length % 2 === 0) {
    throw new Error("Expected odd number of values");
  }
  valuesNs.sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  const half = Math.floor(valuesNs.length / 2);
  // Convert nanoseconds to microseconds (division floors)
  return Number(valuesNs[half] / 1000n);
}

function recordTime(
  runs: Map<string, bigint[]>,
  name: string,
  elapsedNs: bigint
) {
  let measurements = runs.get(name);
  if (measurements === undefined) {
    measurements = [];
    runs.set(name, measurements);
  }
  measurements.push(elapsedNs);
}

function displaySecFromNs(deltaNs: bigint) {
  const sec = deltaNs / 1_000_000_000n;
  return roundToThreeDecimals(Number(sec));
}

function displaySecFromUs(deltaUs: number) {
  const sec = deltaUs / 1_000_000;
  return roundToThreeDecimals(sec);
}

function roundToThreeDecimals(n: number): number {
  return Math.round(n * 1000) / 1000;
}

export async function setupRepo(
  repoData: RepoData,
  tool: "hardhat" | "forge",
  cleanFirst: boolean = false
): Promise<string> {
  const repoNameRegex = /\/([^\/]+)\.git$/;
  const match = repoData.url.match(repoNameRegex);
  if (match === null) {
    throw new Error(`Invalid repo URL: ${repoData.url}`);
  }

  // Use separate directories for the different tools, as both can modify the artifacts directory
  const repoPath = path.join(
    dirName(import.meta.url),
    "..",
    "repos",
    tool,
    match[1]
  );

  if (cleanFirst) {
    fs.rmSync(repoPath, { recursive: true, force: true });
  }

  // Ensure directory exists
  if (!fs.existsSync(repoPath)) {
    await simpleGit().clone(repoData.url, repoPath, [
      "--recurse-submodules",
      "--depth",
      "1",
    ]);
  }

  const git = simpleGit(repoPath);
  await git.fetch(["--depth", "1", "origin", repoData.commit]);
  await git.checkout(repoData.commit);

  if (repoData.patchFile !== undefined) {
    const patchFile = path.join(
      dirName(import.meta.url),
      "..",
      "patches",
      repoData.patchFile
    );
    try {
      await git.raw(["apply", patchFile]);
    } catch (e) {
      if (
        !(e instanceof Error) ||
        // Patch will fail on subsequent runs unless the repo was cleaned first
        (!cleanFirst && !e.toString().toLowerCase().includes("patch failed"))
      ) {
        throw e;
      }
    }
  }

  await execAsync("npm install", { cwd: repoPath });

  return repoPath;
}

async function createSolidityTestsInput(repoPath: string) {
  if (!path.isAbsolute(repoPath)) {
    // If repo path is not absolute, assume it's relative to the current working directory
    repoPath = path.join(process.cwd(), repoPath);
  }

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
    "l1",
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
