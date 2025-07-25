import { createRequire } from "module";

const require = createRequire(import.meta.url);

import { bytesToHex, privateToAddress, toBytes } from "@ethereumjs/util";
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

import { runForgeStdTests, runSolidityTests } from "./solidity-tests.js";

const {
  createHardhatNetworkProvider,
} = require("hardhat2/internal/hardhat-network/provider/provider.js");

const SCENARIOS_DIR = "../../../scenarios/";
const SCENARIO_SNAPSHOT_NAME = "snapshot.json";
const NEPTUNE_MAX_MIN_FAILURES = 1.05;

interface ParsedArguments {
  command: "benchmark" | "verify" | "report" | "solidity-tests";
  grep?: string;
  repo?: string;
  // eslint-disable-next-line @typescript-eslint/naming-convention
  benchmark_output: string;
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
    choices: ["benchmark", "verify", "report", "solidity-tests"],
    help: "Whether to run a benchmark, verify that there are no regressions or create a report for `github-action-benchmark`",
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
    help: "Path to a repo to execute for Solidity tests. Defaults to `forge-std` that is checked out automatically.",
  });
  const args: ParsedArguments = parser.parse_args();

  // if --benchmark-output is relative, resolve it relatively to cwd
  // to reduce ambiguity
  const benchmarkOutputPath = path.resolve(
    process.cwd(),
    args.benchmark_output
  );

  let results: BenchmarkScenarioRpcCalls | undefined;
  if (args.command === "benchmark") {
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
  } else if (args.command === "verify") {
    const success = await verify(benchmarkOutputPath);
    process.exit(success ? 0 : 1);
  } else if (args.command === "report") {
    await report(benchmarkOutputPath);
    await flushStdout();
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
      await runSolidityTests(context, L1_CHAIN_TYPE, args.repo, args.grep);
    } else {
      await runForgeStdTests(context, L1_CHAIN_TYPE, benchmarkOutputPath);
    }
  } else {
    const _exhaustiveCheck: never = args.command;
  }
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
      "benchmark",
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
