import { createRequire } from "module";

const require = createRequire(import.meta.url);

import { bytesToHex, privateToAddress, toBytes } from "@ethereumjs/util";
import Papa from "papaparse";
import {
  EdrContext,
  L1_CHAIN_TYPE,
  l1SolidityTestRunnerFactory,
} from "@nomicfoundation/edr";
import { ArgumentParser } from "argparse";
import child_process, { SpawnSyncReturns } from "child_process";
import fs from "fs";
import _ from "lodash";
import path from "path";
import readline from "readline";
import zlib from "zlib";
import { dirName } from "@nomicfoundation/edr-helpers";

import {
  runSolidityTestsBenchmark,
  runSolidityTests,
  runForgeTests,
  setupRepo,
  REPOS,
} from "./solidity-tests.js";

const {
  createHardhatNetworkProvider,
} = require("hardhat2/internal/hardhat-network/provider/provider.js");

const SCENARIOS_DIR = "../../../scenarios/";
const SCENARIO_SNAPSHOT_NAME = "snapshot.json";
const NEPTUNE_MAX_MIN_FAILURES = 1.05;

interface ParsedArguments {
  command:
    | "provider-benchmark"
    | "verify-provider-benchmark"
    | "report-provider-benchmark"
    | "solidity-tests-benchmark"
    | "solidity-tests"
    | "compare-forge"
    | "report-forge";
  grep?: string;
  repo?: string;
  count?: number;
  // eslint-disable-next-line @typescript-eslint/naming-convention
  benchmark_output: string;
  // eslint-disable-next-line @typescript-eslint/naming-convention
  csv_output?: string;
  // eslint-disable-next-line @typescript-eslint/naming-convention
  csv_input?: string;
  // eslint-disable-next-line @typescript-eslint/naming-convention
  forge_path?: string;
}

interface BenchmarkScenarioResult {
  name: string;
  result: BenchmarkResult;
}

interface BenchmarkResult {
  timeMs: number;
  failures: number[];
}

interface BenchmarkScenarioRpcCalls {
  rpcCallResults: any[];
  rpcCallErrors: any[];
}

interface BenchmarkScenarioReport {
  name: string;
  unit: string;
  value: number;
}

interface Scenario {
  requests: any[];
  config?: any;
}

async function main() {
  const parser = new ArgumentParser({
    description: "Scenario benchmark runner",
  });
  parser.add_argument("command", {
    choices: [
      "provider-benchmark",
      "verify-provider-benchmark",
      "report-provider-benchmark",
      "solidity-tests-benchmark",
      "solidity-tests",
      "compare-forge",
      "report-forge",
    ],
    help: "Whether to run provider benchmarks, verify that there are no regressions, create a report for `github-action-benchmark`, run EDR solidity tests benchmark, compare EDR vs Forge tests, or generate a report from compare-forge output",
  });
  parser.add_argument("-g", "--grep", {
    type: "str",
    help: "Only execute the scenarios or Solidity test paths that contain the given string",
  });
  parser.add_argument("-o", "--benchmark-output", {
    type: "str",
    default: "./benchmark-output.json",
    help: "Where to save the benchmark output file",
  });
  parser.add_argument("-r", "--repo", {
    type: "str",
    help: "Path to a repo to execute for Solidity tests. For the `solidity-tests` command, defaults to `forge-std` that is checked out automatically. For the `compare-forge` command defaults to all supported repos.",
  });
  parser.add_argument("-c", "--count", {
    type: "int",
    default: 3,
    help: "Number of times to run each test suite for compare-forge command (default: 3)",
  });
  parser.add_argument("--csv-output", {
    type: "str",
    help: "Path to save CSV output for compare-forge command",
  });
  parser.add_argument("--csv-input", {
    type: "str",
    help: "Path to input CSV file for report-forge command",
  });
  parser.add_argument("--forge-path", {
    type: "str",
    help: "Path to forge executable (default: 'forge')",
  });
  const args: ParsedArguments = parser.parse_args();

  // if --benchmark-output is relative, resolve it relatively to cwd
  // to reduce ambiguity
  const benchmarkOutputPath = path.resolve(
    process.cwd(),
    args.benchmark_output
  );

  let results: BenchmarkScenarioRpcCalls | undefined;
  if (args.command === "provider-benchmark") {
    if (args.grep !== undefined) {
      for (const scenarioFileName of getScenarioFileNames()) {
        if (scenarioFileName.includes(args.grep)) {
          // We store the results to avoid GC
          // eslint-disable-next-line @typescript-eslint/no-unused-vars
          results = await benchmarkScenario(scenarioFileName);
        }
      }
    } else {
      await benchmarkAllScenarios(benchmarkOutputPath);
    }
    await flushStdout();
  } else if (args.command === "verify-provider-benchmark") {
    const success = await verify(benchmarkOutputPath);
    process.exit(success ? 0 : 1);
  } else if (args.command === "report-provider-benchmark") {
    await report(benchmarkOutputPath);
    await flushStdout();
  } else if (args.command === "solidity-tests-benchmark") {
    await runSolidityTestsBenchmark(benchmarkOutputPath);
  } else if (args.command === "solidity-tests") {
    // Only construct an EdrContext for solidity tests, because the JSON-RPC
    // benchmarks still depend on a singleton Hardhat network provider that
    // is created in the `createHardhatNetworkProvider` function.
    const context = new EdrContext();
    await context.registerSolidityTestRunnerFactory(
      L1_CHAIN_TYPE,
      l1SolidityTestRunnerFactory()
    );

    if (args.repo !== undefined) {
      const csvResults = await runSolidityTests(
        context,
        L1_CHAIN_TYPE,
        args.repo,
        args.grep
      );
      console.log(csvResults);
    } else {
      console.error("Error: --repo is required for solidity-tests command");
      process.exit(1);
    }
  } else if (args.command === "compare-forge") {
    if (args.csv_output === undefined) {
      console.error(
        "Error: --csv-output is required for compare-forge command"
      );
      process.exit(1);
    }
    if (args.forge_path === undefined) {
      console.error(
        "Error: --forge-path is required for compare-forge command"
      );
      process.exit(1);
    }

    if (args.repo !== undefined) {
      await runCompareTests(
        args.repo,
        args.count!,
        args.csv_output,
        /* append */ false,
        args.forge_path
      );
    } else {
      let i = 0;
      for (const repo of Object.keys(REPOS)) {
        await runCompareTests(
          repo,
          args.count!,
          args.csv_output,
          /* append */ i > 0,
          args.forge_path
        );
        i += 1;
      }
    }
  } else if (args.command === "report-forge") {
    if (args.csv_input === undefined) {
      console.error("Error: --csv-input is required for report-forge command");
      process.exit(1);
    }

    const reportResults = await generateForgeReport(args.csv_input);
    console.log(reportResults);
  } else {
    const _exhaustiveCheck: never = args.command;
  }
}

async function repoArgToRepoPath(
  repo: string,
  tool: "hardhat" | "forge"
): Promise<string> {
  const repoData = REPOS[repo];
  if (repoData === undefined) {
    throw new Error(
      `Repo '${repo}' not found. Possible options: ${Object.keys(REPOS).join(", ")} or a local repo path`
    );
  }
  return setupRepo(repoData, tool);
}

async function report(benchmarkResultPath: string) {
  const benchmarkResult: Record<string, BenchmarkResult> = JSON.parse(
    fs.readFileSync(benchmarkResultPath, "utf-8")
  );

  let totalTime = 0;
  const reports: BenchmarkScenarioReport[] = [];
  for (const [scenarioName, scenarioResult] of Object.entries(
    benchmarkResult
  )) {
    reports.push({
      name: scenarioName,
      unit: "ms",
      value: scenarioResult.timeMs,
    });
    totalTime += scenarioResult.timeMs;
  }
  reports.unshift({
    name: "All Scenarios",
    unit: "ms",
    value: totalTime,
  });

  console.log(JSON.stringify(reports));
}

async function verify(benchmarkResultPath: string) {
  let success = true;
  const benchmarkResult = JSON.parse(
    fs.readFileSync(benchmarkResultPath, "utf-8")
  );
  const snapshotResult = JSON.parse(
    fs.readFileSync(
      path.join(getScenariosDir(), SCENARIO_SNAPSHOT_NAME),
      "utf-8"
    )
  );

  for (const scenarioName of Object.keys(snapshotResult)) {
    // TODO https://github.com/NomicFoundation/edr/issues/365
    if (scenarioName.includes("neptune-mutual")) {
      const snapshotCount = snapshotResult[scenarioName].failures.length;
      const actualCount = benchmarkResult[scenarioName].failures.length;
      const ratio =
        Math.max(snapshotCount, actualCount) /
        Math.min(snapshotCount, actualCount);

      if (ratio > NEPTUNE_MAX_MIN_FAILURES) {
        console.error(
          `Snapshot failure for ${scenarioName} with max/min failure ratio`,
          ratio
        );
        success = false;
      }

      continue;
    }

    const snapshotFailures = new Set(snapshotResult[scenarioName].failures);
    const benchFailures = new Set(benchmarkResult[scenarioName].failures);

    if (!_.isEqual(snapshotFailures, benchFailures)) {
      success = false;
      const shouldFail = setDifference(snapshotFailures, benchFailures);
      const shouldNotFail = setDifference(benchFailures, snapshotFailures);

      // We're logging to stderr so that it doesn't pollute stdout where we
      // write the result
      console.error(`Snapshot failure for ${scenarioName}`);

      if (shouldFail.size > 0) {
        console.error(
          `Scenario ${scenarioName} should fail at indexes ${Array.from(
            shouldFail
          ).sort()}`
        );
      }

      if (shouldNotFail.size > 0) {
        console.error(
          `Scenario ${scenarioName} should not fail at indexes ${Array.from(
            shouldNotFail
          ).sort()}`
        );
      }
    }
  }

  if (success) {
    console.error("Benchmark result matches snapshot");
  }

  return success;
}

// From https://stackoverflow.com/a/66512466
function setDifference<T>(a: Set<T>, b: Set<T>): Set<T> {
  return new Set(Array.from(a).filter((item) => !b.has(item)));
}

async function benchmarkAllScenarios(outPath: string) {
  const result: any = {};
  let totalTime = 0;
  let totalFailures = 0;
  for (const scenarioFileName of getScenarioFileNames()) {
    const args = [
      "--noconcurrent_sweeping",
      "--noconcurrent_recompilation",
      "--max-old-space-size=28000",
      "--import",
      "tsx",
      "src/index.ts",
      "provider-benchmark",
      "-g",
      scenarioFileName,
    ];

    let processResult: SpawnSyncReturns<string> | undefined;
    try {
      const scenarioResults: BenchmarkScenarioResult[] = [];
      const iterations = numIterations(scenarioFileName);
      for (let i = 0; i < iterations; i++) {
        // Run in subprocess with grep to simulate Hardhat test runner behaviour
        // where there is one provider per process
        processResult = child_process.spawnSync(process.argv[0], args, {
          shell: true,
          timeout: 60 * 60 * 1000,
          // Pipe stdout, proxy the rest
          stdio: [process.stdin, "pipe", process.stderr],
          encoding: "utf-8",
        });
        const resultFromStdout: any = JSON.parse(processResult.stdout);
        scenarioResults.push(resultFromStdout);
      }
      const scenarioResult = medianOfResults(scenarioResults);
      totalTime += scenarioResult.result.timeMs;
      totalFailures += scenarioResult.result.failures.length;
      result[scenarioResult.name] = scenarioResult.result;
    } catch (e) {
      console.error(e);
      if (processResult !== undefined) {
        console.error(processResult.stdout);
      }
      throw e;
    }
  }

  fs.writeFileSync(outPath, JSON.stringify(result) + "\n");

  // Log info to stderr so that it doesn't pollute stdout where we write the
  // result
  console.error(
    `Total time ${
      Math.round(100 * (totalTime / 1000)) / 100
    } seconds with ${totalFailures} failures.`
  );

  console.error(`Benchmark results written to ${outPath}`);
}

function numIterations(scenarioName: string): number {
  // Run fast scenarios repeatedly to get more reliable results
  if (scenarioName.includes("safe-contracts")) {
    return 15;
  } else if (
    scenarioName.includes("seaport") ||
    scenarioName.includes("uniswap")
  ) {
    return 11;
  } else if (
    scenarioName.includes("openzeppelin") ||
    scenarioName.includes("rocketpool")
  ) {
    return 7;
  } else if (scenarioName.includes("neptune-mutual")) {
    return 5;
  } else {
    return 3;
  }
}

function medianOfResults(results: BenchmarkScenarioResult[]) {
  if (results.length === 0) {
    throw new Error("No results to calculate median");
  }
  const sorted = results.sort((a, b) => a.result.timeMs - b.result.timeMs);
  const middle = Math.floor(sorted.length / 2);
  return sorted[middle];
}

async function benchmarkScenario(
  scenarioFileName: string
): Promise<BenchmarkScenarioRpcCalls> {
  const { config, requests } = await loadScenario(scenarioFileName);
  const name = path.basename(scenarioFileName).split(".")[0];
  console.error(`Running ${name} scenario`);

  const start = performance.now();

  const provider = await createHardhatNetworkProvider(config.providerConfig, {
    enabled: config.loggerEnabled,
  });

  const failures = [];
  const rpcCallResults = [];
  const rpcCallErrors = [];

  for (let i = 0; i < requests.length; i += 1) {
    try {
      const rpcCallResult = await provider.request(requests[i]);
      rpcCallResults.push(rpcCallResult);
    } catch (e) {
      rpcCallErrors.push(e);
      failures.push(i);
    }
  }

  const timeMs = performance.now() - start;

  console.error(
    `${name} finished in ${
      Math.round(100 * (timeMs / 1000)) / 100
    } seconds with ${failures.length} failures.`
  );

  const result: BenchmarkScenarioResult = {
    name,
    result: {
      timeMs,
      failures,
    },
  };
  console.log(JSON.stringify(result));

  // Return this to avoid gc
  return { rpcCallResults, rpcCallErrors };
}

async function loadScenario(scenarioFileName: string): Promise<Scenario> {
  const result: Scenario = {
    requests: [],
  };
  let i = 0;
  const filePath = path.join(getScenariosDir(), scenarioFileName);
  for await (const line of readFile(filePath)) {
    const parsed = JSON.parse(line);
    if (i === 0) {
      result.config = preprocessConfig(parsed);
    } else {
      result.requests.push(parsed);
    }
    i += 1;
  }
  return result;
}

function preprocessConfig(config: any) {
  // EDR serializes None as null to json, but Hardhat expects it to be undefined
  const removeNull = (obj: any) =>
    _.transform(obj, (acc: any, value: any, key: any) => {
      if (_.isObject(value)) {
        acc[key] = removeNull(value);
      } else if (!_.isNull(value)) {
        acc[key] = value;
      }
    });
  config = removeNull(config);

  config.providerConfig.initialDate = new Date(
    config.providerConfig.initialDate
  );

  config.providerConfig.hardfork = normalizeHardfork(
    config.providerConfig.hardfork
  );

  const genesisState = new Map<string, any>(
    Object.entries(config.providerConfig.genesisState)
  );

  // In EDR, all state modifications are in the genesis state, so we need to
  // retrieve the balance for the owned accounts from there.
  config.providerConfig.genesisAccounts =
    config.providerConfig.ownedAccounts.map((secretKey: string) => {
      const address = bytesToHex(privateToAddress(toBytes(secretKey)));
      const balance = genesisState.get(address)?.balance ?? 0n;

      return { balance, privateKey: secretKey };
    });
  delete config.providerConfig.ownedAccounts;

  config.providerConfig.automine = config.providerConfig.mining.autoMine;
  config.providerConfig.mempoolOrder =
    config.providerConfig.mining.memPool.order.toLowerCase();
  config.providerConfig.intervalMining =
    config.providerConfig.mining.interval ?? 0;
  delete config.providerConfig.mining;

  config.providerConfig.throwOnCallFailures =
    config.providerConfig.bailOnCallFailure;
  delete config.providerConfig.bailOnCallFailure;
  config.providerConfig.throwOnTransactionFailures =
    config.providerConfig.bailOnTransactionFailure;
  delete config.providerConfig.bailOnTransactionFailure;

  const chains: any = new Map();
  for (const key of Object.keys(config.providerConfig.chainOverrides)) {
    const hardforkHistory = new Map();
    const chainConfig = config.providerConfig.chainOverrides[key];
    for (const { condition, hardfork } of chainConfig.hardforkActivations) {
      if (!_.isUndefined(condition.timestamp)) {
        throw new Error("Hardfork activations by timestamp are not supported");
      } else if (!_.isUndefined(condition.block)) {
        hardforkHistory.set(normalizeHardfork(hardfork), condition.block);
      } else {
        throw new Error("Unsupported hardfork condition");
      }
    }

    chains.set(Number(key), { hardforkHistory });
  }
  config.providerConfig.chains = chains;

  if (!_.isUndefined(config.providerConfig.fork)) {
    config.providerConfig.forkConfig = config.providerConfig.fork;
    delete config.providerConfig.fork;
  }

  config.providerConfig.minGasPrice = BigInt(config.providerConfig.minGasPrice);
  config.providerConfig.enableRip7212 = false;

  return config;
}

function normalizeHardfork(hardfork: string) {
  hardfork = _.camelCase(hardfork);
  if (hardfork === "frontier") {
    hardfork = "chainstart";
  } else if (hardfork === "daoFork") {
    hardfork = "dao";
  } else if (hardfork === "spurious") {
    hardfork = "spuriousDragon";
  } else if (hardfork === "tangerine") {
    hardfork = "tangerineWhistle";
  }
  return hardfork;
}

// From https://stackoverflow.com/a/65015455/2650622
function readFile(pathToRead: string) {
  let stream: any = fs.createReadStream(pathToRead);

  if (/\.gz$/i.test(pathToRead)) {
    stream = stream.pipe(zlib.createGunzip());
  }

  return readline.createInterface({
    input: stream,
    crlfDelay: Infinity,
  });
}

function getScenariosDir() {
  return path.join(dirName(import.meta.url), SCENARIOS_DIR);
}

function getScenarioFileNames(): string[] {
  const scenariosDir = path.join(dirName(import.meta.url), SCENARIOS_DIR);
  const scenarioFiles = fs.readdirSync(scenariosDir);
  scenarioFiles.sort();
  return scenarioFiles.filter((fileName) => fileName.endsWith(".jsonl.gz"));
}

async function runSolidityTestsInSubprocess(repo: string): Promise<string> {
  const args = [
    "--noconcurrent_sweeping",
    "--noconcurrent_recompilation",
    "--max-old-space-size=28000",
    "--import",
    "tsx",
    "src/index.ts",
    "solidity-tests",
    "--repo",
    repo,
  ];

  const processResult = child_process.spawnSync(process.argv[0], args, {
    shell: true,
    timeout: 60 * 60 * 1000, // 1 hour timeout
    stdio: [process.stdin, "pipe", process.stderr],
    encoding: "utf-8",
  });

  if (processResult.error !== undefined) {
    throw new Error(`Failed to run EDR tests: ${processResult.error.message}`);
  }

  if (processResult.status !== 0) {
    throw new Error(`EDR tests failed with exit code ${processResult.status}`);
  }

  // Return CSV output from stdout
  return processResult.stdout.trim();
}

async function runCompareTests(
  repo: string,
  count: number,
  csvOutputPath: string,
  append: boolean,
  forgePath: string
) {
  const allCsvData: any[] = [];

  console.error(
    `Running EDR and Forge tests ${count} times each for ${repo} (alternating)...`
  );

  const forgeRepoPath = await repoArgToRepoPath(repo, "forge");
  const hardhatRepoPath = await repoArgToRepoPath(repo, "hardhat");

  // Alternate between EDR and Forge for each run
  for (let i = 0; i < count; i++) {
    // Run EDR test in subprocess
    console.error(`EDR run ${i + 1}/${count}`);
    const edrResultsCsv = await runSolidityTestsInSubprocess(hardhatRepoPath);

    // Parse EDR CSV results
    const edrParseResult = Papa.parse(edrResultsCsv, {
      header: true,
      skipEmptyLines: true,
    });

    // Add run number to each row
    for (const row of edrParseResult.data as any[]) {
      allCsvData.push({
        ...row,
        runNumber: (i + 1).toString(),
      });
    }

    // Run Forge test
    console.error(`Forge run ${i + 1}/${count}`);
    const forgeResultsCsv = await runForgeTests(forgeRepoPath, forgePath);

    // Parse Forge CSV results
    const forgeParseResult = Papa.parse(forgeResultsCsv, {
      header: true,
      skipEmptyLines: true,
    });

    // Add run number to each row
    for (const row of forgeParseResult.data as any[]) {
      allCsvData.push({
        ...row,
        runNumber: (i + 1).toString(),
      });
    }
  }

  // Save merged results to the CSV file
  const csvContent = Papa.unparse(allCsvData, { header: !append });
  if (append) {
    fs.appendFileSync(csvOutputPath, "\r\n");
    fs.appendFileSync(csvOutputPath, csvContent);
  } else {
    fs.writeFileSync(csvOutputPath, csvContent);
  }

  console.error(`CSV results saved to ${csvOutputPath}`);
}

interface TestGroup {
  edr: bigint[];
  forge: bigint[];
  failed: boolean;
  testType: string;
}

interface TestTypeTotal {
  edr: bigint;
  forge: bigint;
}

interface RepoStats {
  unit: TestTypeTotal;
  fuzz: TestTypeTotal;
  invariant: TestTypeTotal;
  successfulTestCount: number;
  failedTestCount: number;
}

interface CompareForgeRow {
  repo: string;
  testSuiteSource: string;
  testSuiteName: string;
  testName: string | undefined;
  testType: string;
  durationNs: string;
  executor: string;
  outcome: string;
  runNumber: string;
}

// Compares the sum the test execution times. Shouldn't compare suite execution times as there is unpredictability due to nested parallelism in test suites.
async function generateForgeReport(csvInputPath: string): Promise<string> {
  const csvContent = fs.readFileSync(csvInputPath, "utf-8");

  const parseResult = Papa.parse<CompareForgeRow>(csvContent, {
    header: true,
    skipEmptyLines: true,
  });

  if (parseResult.errors.length > 0) {
    throw new Error(
      `CSV parsing errors: ${parseResult.errors.map((e) => e.message).join(", ")}`
    );
  }

  // Parse data rows and filter only actual tests (rows with test names) that succeeded
  const testRows = parseResult.data.filter((row) => {
    // Only include rows that have a test name (exclude suite totals and overall totals)
    // and where the test succeeded
    return row.testName !== undefined && row.testName.trim() !== "";
  });

  // Group tests by test identification to calculate medians
  const testGroups = new Map<string, TestGroup>();

  for (const row of testRows) {
    const testKey = `${row.repo}|${row.testSuiteSource}|${row.testSuiteName}|${row.testName}`;

    if (!testGroups.has(testKey)) {
      testGroups.set(testKey, {
        edr: [],
        forge: [],
        failed: false,
        testType: row.testType,
      });
    }

    const group = testGroups.get(testKey)!;
    group.failed = group.failed || row.outcome !== "success";

    const duration = BigInt(row.durationNs);

    if (row.executor === "edr") {
      group.edr.push(duration);
    } else if (row.executor === "forge") {
      group.forge.push(duration);
    } else {
      throw new Error(`Unknown executor for row: '${row}'`);
    }
  }

  const repoStats = new Map<string, RepoStats>();

  for (const testKey of Array.from(testGroups.keys()).sort()) {
    const repo = testKey.split("|")[0];
    const durations = testGroups.get(testKey)!;

    if (!repoStats.has(repo)) {
      repoStats.set(repo, {
        unit: { edr: 0n, forge: 0n },
        fuzz: { edr: 0n, forge: 0n },
        invariant: { edr: 0n, forge: 0n },
        successfulTestCount: 0,
        failedTestCount: 0,
      });
    }
    const stats: RepoStats = repoStats.get(repo)!;

    // Only include tests where both EDR and Forge have successful results
    if (durations.failed) {
      stats.failedTestCount += 1;
      continue;
    }

    if (
      durations.edr.length !== durations.forge.length ||
      durations.edr.length === 0
    ) {
      throw new Error(
        `Expected durations to be the same and non-zero for ${testKey}, instead they are: ${durations.edr.length} and ${durations.forge.length} for EDR and Forge, respectively.`
      );
    }

    const edrMedian = calculateMedianBigInt(durations.edr);
    const forgeMedian = calculateMedianBigInt(durations.forge);

    stats.successfulTestCount += 1;

    // Add to test type totals
    if (durations.testType === "unit") {
      stats.unit.edr += edrMedian;
      stats.unit.forge += forgeMedian;
    } else if (durations.testType === "fuzz") {
      stats.fuzz.edr += edrMedian;
      stats.fuzz.forge += forgeMedian;
    } else if (durations.testType === "invariant") {
      stats.invariant.edr += edrMedian;
      stats.invariant.forge += forgeMedian;
    }
  }

  const reportRows = [];
  for (const [repo, stats] of repoStats) {
    // Calculate overall totals from test type totals
    const edrTotalNs = stats.unit.edr + stats.fuzz.edr + stats.invariant.edr;
    const forgeTotalNs =
      stats.unit.forge + stats.fuzz.forge + stats.invariant.forge;

    // Calculate ratios
    const totalRatio =
      forgeTotalNs > 0n ? Number((edrTotalNs * 100n) / forgeTotalNs) / 100 : 0;
    const unitRatio =
      stats.unit.forge > 0n
        ? Number((stats.unit.edr * 100n) / stats.unit.forge) / 100
        : 0;
    const fuzzRatio =
      stats.fuzz.forge > 0n
        ? Number((stats.fuzz.edr * 100n) / stats.fuzz.forge) / 100
        : 0;
    const invariantRatio =
      stats.invariant.forge > 0n
        ? Number((stats.invariant.edr * 100n) / stats.invariant.forge) / 100
        : 0;

    reportRows.push({
      repo,
      successful_tests: stats.successfulTestCount.toString(),
      failed_tests: stats.failedTestCount.toString(),
      total_ratio: totalRatio.toString(),
      unit_ratio: unitRatio.toString(),
      fuzz_ratio: fuzzRatio.toString(),
      invariant_ratio: invariantRatio.toString(),
    });
  }

  return Papa.unparse(reportRows);
}

function calculateMedianBigInt(values: bigint[]): bigint {
  const sorted = [...values].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  const mid = Math.floor(sorted.length / 2);

  if (sorted.length % 2 === 0) {
    return (sorted[mid - 1] + sorted[mid]) / 2n;
  } else {
    return sorted[mid];
  }
}

async function flushStdout() {
  return new Promise((resolve) => {
    process.stdout.write("", resolve);
  });
}

main()
  .then(() => {
    process.exit(0);
  })
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
