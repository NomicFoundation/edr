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
import child_process, { exec } from "child_process";
import { promisify } from "util";
import { stringify } from "csv-stringify/sync";

const execAsync = promisify(exec);
import {
  buildSolidityTestsInput,
  dirName,
  runAllSolidityTests,
} from "@nomicfoundation/edr-helpers";
import {
  FsAccessPermission,
  SuiteResult,
  TestStatus,
  EdrContext,
  StandardTestKind,
  FuzzTestKind,
  InvariantTestKind,
  L1_CHAIN_TYPE,
  l1SolidityTestRunnerFactory,
  l1HardforkLatest,
  l1HardforkToString,
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
    url: "https://github.com/foundry-rs/forge-std.git",
    commit: "3f999523613ab5454a5c4ae4abeaa8ea2ba7bcae",
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

  // Convert to CSV string
  return stringify(csvData, { header: true });
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

  // Convert to CSV string
  return stringify(csvData, { header: true });
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
  Standard?: Record<string, never>;
  Unit?: Record<string, never>;
}

function getForgeTestType(kind: ForgeTestKind): string {
  if (kind.Fuzz !== undefined) {
    return "fuzz";
  } else if (kind.Invariant !== undefined) {
    return "invariant";
  } else if (kind.Standard !== undefined || kind.Unit !== undefined) {
    return "unit";
  } else {
    throw new Error(`Unknown test type: ${JSON.stringify(kind)}`);
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
  const SEC_IN_NS = 1_000_000_000n;
  const sec = deltaNs / SEC_IN_NS;
  const remainder = deltaNs % SEC_IN_NS;
  // Floor to 3 decimal places. Can't round due to BigInt and converting ns to
  // Number risks losing precision as the max safe int for Number is 2^53 – 1.
  return `${sec}.${remainder.toString().slice(0, 3)}`;
}

function displaySecFromUs(deltaUs: number) {
  const sec = deltaUs / 1_000_000;
  // Floor to 3 decimal places in order to make it consistent with displaySecFromNs
  return Math.floor(sec * 1000) / 1000;
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

  // The shallow clone didn't fetch submodules, so update them.
  await git.raw([
    "submodule",
    "update",
    "--init",
    "--recursive",
    "--depth",
    "1",
  ]);

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

  try {
    await execAsync("npm install", { cwd: repoPath });
  } catch {
    console.error(
      `npm install failed for ${repoPath}, retrying with --ignore-scripts`
    );
    await execAsync("npm install --ignore-scripts", { cwd: repoPath });
  }

  return repoPath;
}

async function createSolidityTestsInput(repoPath: string, verbosity = 0) {
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

  const { artifacts, testSuiteIds, tracingConfig, testSourcePaths } =
    await buildSolidityTestsInput(hre);
  const solidityTestsConfig =
    await solidityTestConfigToSolidityTestRunnerConfigArgs({
      chainType: "l1",
      projectRoot: repoPath,
      config: userConfig.solidityTest,
      verbosity,
      observability: undefined,
      testPattern: undefined,
      generateGasReport: false,
    });
  // TODO: move to solidityTestConfigToSolidityTestRunnerConfigArgs after it's updated in Hardhat
  solidityTestsConfig.hardfork = l1HardforkToString(l1HardforkLatest());
  // Temporary workaround for `testFuzz_AssumeNotPrecompile` in forge-std which assumes no predeploys on mainnet.
  solidityTestsConfig.localPredeploys = undefined;

  solidityTestsConfig.projectRoot = repoPath;
  solidityTestsConfig.rpcCachePath = RPC_CACHE_PATH;
  // Absolute paths of the test sources, for inline-config parsing.
  solidityTestsConfig.testSourcePaths = testSourcePaths;
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
      if (r.status !== TestStatus.Success) {
        failed.add(`${res.id.name} ${r.name} ${r.status} reason:\n${r.reason}`);
      }
    }
  }
  if (failed.size !== 0) {
    console.error(failed);
    throw new Error(`Some tests failed`);
  }
}

// ---------------------------------------------------------------------------
// Memory benchmark
//
// Call-trace arena retention is a function of Hardhat's verbosity, which maps
// to `includeTraces`/`collectStackTraces`:
//
//   verbosity <= 2  ->  IncludeTraces.None    + CollectStackTraces.OnFailure
//   verbosity == 3  ->  IncludeTraces.Failing + CollectStackTraces.Always
//   verbosity >= 4  ->  IncludeTraces.All     + CollectStackTraces.Always
//
// Peak RSS is the metric because the arenas live in Rust while the napi
// `TestResult` objects that reference them are held by JS.
// ---------------------------------------------------------------------------

/** The verbosity levels that produce distinct trace-retention behaviour. */
export const MEMORY_VERBOSITIES = [2, 3, 4];

/**
 * Rayon thread cap for measured children. Test suites (and tests within a
 * suite) run in parallel, and each in-flight suite retains its trace arenas, so
 * peak RSS scales with parallelism. An unbounded run OOMs a 16 GiB machine on
 * the unoptimized baseline (solady at verbosity 3 exceeds 10 GiB), which would
 * leave nothing to compare optimisations against; a fixed cap keeps every
 * (build × repo × verbosity) cell measurable and comparable.
 */
const MEMORY_RAYON_THREADS = "4";

/** Identifies a (repo, verbosity) cell; common to every outcome. */
export interface MemoryCell {
  repo: string;
  verbosity: number;
}

/** What the child prints: the measurement it can observe about itself. */
export interface MemoryMeasurementPayload extends MemoryCell {
  /** Peak resident set size of the whole process, in bytes. */
  peakRssBytes: number;
  elapsedMs: number;
  suiteCount: number;
  testCount: number;
  failureCount: number;
}

/** The child completed and reported its measurement. */
export interface MemoryMeasurement extends MemoryMeasurementPayload {
  kind: "measured";
  /** Peak RSS as reported by `/usr/bin/time -v`, if it was available. */
  peakRssBytesExternal?: number;
}

/**
 * The child was killed (out of memory) before completing. Only the peak GNU
 * time observed from outside is known, and only when it was available; the
 * true requirement is higher either way.
 */
export interface MemoryExhausted extends MemoryCell {
  kind: "oom";
  peakRssBytesExternal?: number;
}

/**
 * The child failed for a reason other than running out of memory. The other
 * cells are still measured, but the benchmark exits unsuccessfully.
 */
export interface MemoryRunError extends MemoryCell {
  kind: "error";
  error: string;
}

/** The outcome of one (repo, verbosity) cell. */
export type MemoryRunResult =
  MemoryMeasurement | MemoryExhausted | MemoryRunError;

/**
 * Child-process entry point: run one repo at one verbosity and report peak RSS
 * on stdout as a single JSON line prefixed with `MEMORY_RESULT `.
 *
 * This must run in its own process: `maxRSS` is a high-water mark that never
 * decreases, so a second measurement in the same process would inherit the
 * first one's peak.
 */
export async function runSolidityTestsMemoryChild(
  context: EdrContext,
  chainType: string,
  repoName: string,
  repoPath: string,
  verbosity: number
): Promise<MemoryMeasurementPayload> {
  const { artifacts, testSuiteIds, tracingConfig, solidityTestsConfig } =
    await createSolidityTestsInput(repoPath, verbosity);

  const startNs = process.hrtime.bigint();
  const [, results] = await runAllSolidityTests(
    context,
    chainType,
    artifacts,
    testSuiteIds,
    tracingConfig,
    solidityTestsConfig
  );
  const elapsedNs = process.hrtime.bigint() - startNs;

  if (results.length === 0) {
    throw new Error(`Didn't run any tests for ${repoName}`);
  }

  // Read the high-water mark *before* dropping the results, and keep `results`
  // alive across the read so neither V8 nor Rust can reclaim the arenas early.
  // `maxRSS` is in kilobytes on Linux.
  const peakRssBytes = process.resourceUsage().maxRSS * 1024;

  let testCount = 0;
  let failureCount = 0;
  for (const suite of results) {
    for (const test of suite.testResults) {
      testCount += 1;
      if (test.status !== TestStatus.Success) {
        failureCount += 1;
      }
    }
  }

  return {
    repo: repoName,
    verbosity,
    peakRssBytes,
    elapsedMs: Number(elapsedNs / 1_000_000n),
    suiteCount: results.length,
    testCount,
    failureCount,
  };
}

const MEMORY_RESULT_PREFIX = "MEMORY_RESULT ";

/** Print a child result in the form the driver parses. */
export function printMemoryResult(result: MemoryMeasurementPayload) {
  console.log(MEMORY_RESULT_PREFIX + JSON.stringify(result));
}

/**
 * Parameters the driver passes to `solidity-tests-memory-child.ts`, as a
 * single JSON argument.
 */
export interface MemoryChildParams {
  repo: string;
  repoPath: string;
  verbosity: number;
  /** Only compile the repo (to warm the artifact cache); don't measure. */
  compileOnly?: boolean;
}

function hasGnuTime(): boolean {
  try {
    child_process.execFileSync("/usr/bin/time", ["-v", "true"], {
      stdio: "ignore",
    });
    return true;
  } catch {
    return false;
  }
}

/** Extract `Maximum resident set size (kbytes): N` from `/usr/bin/time -v`. */
function parseGnuTimeMaxRss(stderr: string): number | undefined {
  const match = stderr.match(/Maximum resident set size \(kbytes\): (\d+)/);
  return match === null ? undefined : Number(match[1]) * 1024;
}

/** Whether a child process exited because it ran out of memory. */
function isOomError(
  code: number | null,
  signal: NodeJS.Signals | null,
  stderr: string
): boolean {
  // The Linux OOM killer terminates processes with SIGKILL.
  if (signal === "SIGKILL") {
    return true;
  }

  // When the child is wrapped in GNU time, the kill hits the grandchild:
  // GNU time itself then exits with 128 + 9 and reports the signal on
  // stderr.
  if (code === 128 + 9 || /Command terminated by signal 9\b/.test(stderr)) {
    return true;
  }

  // V8 aborts the process when its own heap limit is exhausted. Match the
  // fatal-error message rather than the SIGABRT it dies with, which has
  // other causes too.
  return /JavaScript heap out of memory/.test(stderr);
}

/**
 * Driver: run every (repo, verbosity) pair in a fresh child process and collect
 * peak RSS.
 */
export async function runSolidityTestsMemoryBenchmark(
  repoNames: string[],
  verbosities: number[],
  resultsPath: string
): Promise<MemoryRunResult[]> {
  for (const repoName of repoNames) {
    if (REPOS[repoName] === undefined) {
      throw new Error(
        `Unknown repo '${repoName}'. Known repos: ${Object.keys(REPOS).join(", ")}`
      );
    }
  }

  const repoPaths = new Map<string, string>();
  for (const repoName of repoNames) {
    console.error(`setting up ${repoName}...`);
    const repoPath = await setupRepo(REPOS[repoName], "hardhat");
    repoPaths.set(repoName, repoPath);

    // Compile in a throwaway child so the first measured run doesn't pay
    // solc costs that later runs get from the artifact cache.
    console.error(`compiling ${repoName}...`);
    await runCompileOnlyChildProcess(repoName, repoPath);
  }

  const useGnuTime = hasGnuTime();
  if (!useGnuTime) {
    console.error(
      "note: /usr/bin/time not available, relying on process.resourceUsage() alone"
    );
  }

  const results: MemoryRunResult[] = [];
  for (const repoName of repoNames) {
    for (const verbosity of verbosities) {
      console.error(`measuring ${repoName} at verbosity ${verbosity}...`);
      let result: MemoryRunResult;
      try {
        result = await runMemoryChildProcess(
          repoName,
          repoPaths.get(repoName)!,
          verbosity,
          useGnuTime
        );
      } catch (e) {
        // Record the failure and keep measuring the other cells; the caller
        // reports the failed ones.
        const error = e instanceof Error ? e.message : String(e);
        console.error(`  error: ${error}`);
        result = { kind: "error", repo: repoName, verbosity, error };
      }
      if (result.kind !== "error") {
        logMemoryProgress(result);
      }
      results.push(result);
    }
  }

  fs.writeFileSync(resultsPath, JSON.stringify(results, null, 2) + "\n");
  console.error(`saved results to ${resultsPath}`);
  console.log(formatMemoryTable(results));

  return results;
}

/** Compile a repo's contracts + tests without running anything. */
export async function compileSolidityTestsInput(repoPath: string) {
  await createSolidityTestsInput(repoPath);
}

/**
 * Node arguments invoking the internal child entry point with the given
 * parameters. `process.execArgv` carries the driver's node flags over.
 */
function memoryChildNodeArgs(params: MemoryChildParams): string[] {
  const childEntry = path.join(
    dirName(import.meta.url),
    "solidity-tests-memory-child.ts"
  );
  return [...process.execArgv, childEntry, JSON.stringify(params)];
}

function runCompileOnlyChildProcess(
  repoName: string,
  repoPath: string
): Promise<void> {
  const nodeArgs = memoryChildNodeArgs({
    repo: repoName,
    repoPath,
    verbosity: 0,
    compileOnly: true,
  });
  return new Promise((resolve, reject) => {
    const child = child_process.spawn(process.execPath, nodeArgs, {
      cwd: process.cwd(),
      stdio: ["ignore", "inherit", "inherit"],
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`compile child for ${repoName} exited with ${code}`));
      }
    });
  });
}

function runMemoryChildProcess(
  repoName: string,
  repoPath: string,
  verbosity: number,
  useGnuTime: boolean
): Promise<MemoryMeasurement | MemoryExhausted> {
  const nodeArgs = memoryChildNodeArgs({
    repo: repoName,
    repoPath,
    verbosity,
  });

  // Wrap in GNU time where available: it observes the peak RSS externally,
  // which also covers a child that gets OOM-killed before it can report.
  const [command, commandArgs] = useGnuTime
    ? ["/usr/bin/time", ["-v", process.execPath, ...nodeArgs]]
    : [process.execPath, nodeArgs];

  return new Promise((resolve, reject) => {
    const child = child_process.spawn(command, commandArgs, {
      cwd: process.cwd(),
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, RAYON_NUM_THREADS: MEMORY_RAYON_THREADS },
    });

    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => (stdout += chunk));
    child.stderr.on("data", (chunk) => (stderr += chunk));

    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (isOomError(code, signal, stderr)) {
        resolve({
          kind: "oom",
          repo: repoName,
          verbosity,
          peakRssBytesExternal: parseGnuTimeMaxRss(stderr),
        });
        return;
      }

      if (code !== 0) {
        reject(
          new Error(
            `memory child for ${repoName} v${verbosity} exited with ${code}\n${stderr}`
          )
        );
        return;
      }

      const line = stdout
        .split("\n")
        .find((l) => l.startsWith(MEMORY_RESULT_PREFIX));
      if (line === undefined) {
        reject(
          new Error(
            `memory child for ${repoName} v${verbosity} produced no result\n${stdout}\n${stderr}`
          )
        );
        return;
      }

      const measurement: MemoryMeasurementPayload = JSON.parse(
        line.slice(MEMORY_RESULT_PREFIX.length)
      );
      resolve({
        kind: "measured",
        ...measurement,
        peakRssBytesExternal: useGnuTime
          ? parseGnuTimeMaxRss(stderr)
          : undefined,
      });
    });
  });
}

/**
 * The peak RSS to report for a result, or `undefined` when nothing observed
 * one: the externally observed peak where available, since it also covers
 * allocations made after the in-process read.
 */
function reportedPeakRssBytes(
  result: MemoryMeasurement | MemoryExhausted
): number | undefined {
  return result.kind === "measured"
    ? (result.peakRssBytesExternal ?? result.peakRssBytes)
    : result.peakRssBytesExternal;
}

/** Logs one measured or out-of-memory cell to the progress stream. */
function logMemoryProgress(result: MemoryMeasurement | MemoryExhausted) {
  const peak = reportedPeakRssBytes(result);
  switch (result.kind) {
    case "measured":
      console.error(
        `  peak RSS ${displayMiB(peak ?? result.peakRssBytes)}, ` +
          `${displayDuration(result.elapsedMs)}, ${result.testCount} tests ` +
          `(${result.failureCount} failing)`
      );
      break;
    case "oom":
      console.error(
        peak !== undefined
          ? `  out of memory at >= ${displayMiB(peak)}`
          : "  out of memory (peak RSS unknown)"
      );
      break;
    default: {
      const _exhaustiveCheck: never = result;
      throw new Error(`unrecognized memory result: ${JSON.stringify(result)}`);
    }
  }
}

function displayMiB(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

/** Sub-second durations in milliseconds, longer ones in seconds. */
function displayDuration(elapsedMs: number): string {
  if (elapsedMs < 1000) {
    return `${elapsedMs}ms`;
  }
  return `${(elapsedMs / 1000).toFixed(1)}s`;
}

/** Render results as a markdown table: rows are repos, columns are verbosities. */
export function formatMemoryTable(results: MemoryRunResult[]): string {
  const verbosities = Array.from(new Set(results.map((r) => r.verbosity))).sort(
    (a, b) => a - b
  );
  const repos = Array.from(new Set(results.map((r) => r.repo)));

  const header = ["repo", ...verbosities.map((v) => `-${"v".repeat(v)}`)];
  const rows = repos.map((repo) => [
    repo,
    ...verbosities.map((verbosity) => {
      const result = results.find(
        (r) => r.repo === repo && r.verbosity === verbosity
      );
      if (result === undefined) {
        return "—";
      }
      switch (result.kind) {
        case "measured":
          return `${displayMiB(reportedPeakRssBytes(result) ?? result.peakRssBytes)} / ${displayDuration(result.elapsedMs)}`;
        case "oom": {
          const peak = reportedPeakRssBytes(result);
          return peak !== undefined ? `OOM (>= ${displayMiB(peak)})` : "OOM";
        }
        case "error":
          return "error";
        default: {
          const _exhaustiveCheck: never = result;
          throw new Error(
            `unrecognized memory result: ${JSON.stringify(result)}`
          );
        }
      }
    }),
  ]);

  const lines = [
    `| ${header.join(" | ")} |`,
    `|${header.map(() => "---").join("|")}|`,
    ...rows.map((row) => `| ${row.join(" | ")} |`),
  ];
  return lines.join("\n");
}
