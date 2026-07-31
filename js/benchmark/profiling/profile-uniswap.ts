/**
 * Phase-instrumented driver for profiling `hardhat test solidity` on a Hardhat 3
 * project, mirroring the real task action:
 *   hardhat/dist/src/internal/builtin-plugins/solidity-test/task-action.js
 *
 * It reports wall-clock time per component so a flamegraph can be read against
 * a known phase breakdown. Temporary profiling tool -- not part of the repo.
 *
 * Usage:
 *   node --import tsx profile-uniswap.ts [--repo <path>] [--fuzz-runs <n>]
 *                                        [--json <out.json>] [--label <name>]
 */
import path from "node:path";
import fs from "node:fs";
import { performance } from "node:perf_hooks";

import {
  EdrContext,
  L1_CHAIN_TYPE,
  l1SolidityTestRunnerFactory,
  TestStatus,
  type SuiteResult,
} from "@nomicfoundation/edr";
import { runAllSolidityTests } from "@nomicfoundation/edr-helpers";
import { createHardhatRuntimeEnvironment } from "hardhat/hre";
import {
  isTestSuiteArtifact,
  solidityTestConfigToSolidityTestRunnerConfigArgs,
} from "hardhat/internal/builtin-plugins/solidity-test/helpers";
import {
  buildEdrArtifactsWithMetadata,
  getBuildInfosAndOutputs,
} from "hardhat/internal/builtin-plugins/solidity-test/edr-artifacts";
import { ArtifactManagerImplementation } from "hardhat/internal/builtin-plugins/artifacts/artifact-manager";
import { resolveFromRoot } from "@nomicfoundation/hardhat-utils/path";

// `inline-config` is not in Hardhat's package.json "exports" map, so import it
// by absolute path (resolved relative to a module that *is* exported).
async function loadInlineConfig(): Promise<{
  getTestFunctionOverrides: (a: unknown, b: unknown) => unknown;
}> {
  const { pathToFileURL } = await import("node:url");
  const { createRequire } = await import("node:module");
  const require = createRequire(import.meta.url);
  const helpersPath =
    require.resolve("hardhat/internal/builtin-plugins/solidity-test/helpers");
  const target = path.join(
    path.dirname(helpersPath),
    "inline-config",
    "index.js"
  );
  return import(pathToFileURL(target).href) as any;
}

// ---------------------------------------------------------------- args

function parseArgs() {
  const argv = process.argv.slice(2);
  const get = (name: string, fallback?: string) => {
    const i = argv.indexOf(`--${name}`);
    return i === -1 ? fallback : argv[i + 1];
  };
  return {
    repo: path.resolve(get("repo", "/workspaces/migrations/uniswap-v4-core")!),
    fuzzRuns: Number(get("fuzz-runs", "1000")),
    json: get("json"),
    label: get("label", "run"),
    grep: get("grep"),
  };
}

const ARGS = parseArgs();

// ---------------------------------------------------------------- timing

interface Phase {
  name: string;
  ms: number;
  note?: string;
}
const phases: Phase[] = [];

async function timed<T>(
  name: string,
  fn: () => Promise<T> | T,
  note?: (r: T) => string
): Promise<T> {
  const t0 = performance.now();
  const result = await fn();
  const ms = performance.now() - t0;
  phases.push({ name, ms, note: note?.(result) });
  process.stderr.write(
    `  [phase] ${name.padEnd(28)} ${ms.toFixed(1).padStart(10)} ms${
      note !== undefined ? `   (${note(result)})` : ""
    }\n`
  );
  return result;
}

// ---------------------------------------------------------------- config
//
// Translated from /workspaces/migrations/uniswap-v4-core/hardhat.config.ts.
// Defined inline rather than imported so we bind against this monorepo's
// Hardhat (3.4.5 + workspace EDR) instead of the target repo's own copy.
// `fuzz.runs` is parameterised so both fuzz regimes can be profiled without
// touching the target repo.

function userConfig(fuzzRuns: number) {
  return {
    solidity: {
      profiles: {
        default: {
          compilers: [
            {
              version: "0.8.26",
              settings: {
                evmVersion: "cancun",
                optimizer: { enabled: true, runs: 44_444_444 },
                viaIR: true,
                metadata: { bytecodeHash: "none" },
              },
            },
          ],
        },
      },
    },
    paths: {
      sources: "./src",
      tests: "./test",
    },
    test: {
      solidity: {
        ffi: true,
        gasLimit: 300_000_000n,
        allowInternalExpectRevert: true,
        fsPermissions: {
          dangerouslyReadWriteDirectory: [".forge-snapshots/"],
          readDirectory: ["./out", "./test/bin"],
        },
        fuzz: {
          runs: fuzzRuns,
          seed: "0x4444",
        },
      },
    },
  };
}

// ---------------------------------------------------------------- main

async function main() {
  process.stderr.write(
    `\n=== profiling ${ARGS.repo} (label=${ARGS.label}, fuzz.runs=${ARGS.fuzzRuns}) ===\n`
  );

  const totalStart = performance.now();

  const hre = await timed("hre:construct", () =>
    createHardhatRuntimeEnvironment(
      userConfig(ARGS.fuzzRuns) as any,
      {},
      ARGS.repo
    )
  );

  // `splitTestsCompilation` is false by default since Hardhat 3.4.x, so
  // contracts and tests share one artifacts directory and one build task.
  const split = hre.config.solidity.splitTestsCompilation;

  let testRootPaths: string[];
  if (split) {
    await timed("build:contracts", () =>
      hre.tasks.getTask("build").run({ noTests: true, quiet: true })
    );
    const r: any = await timed("build:tests", () =>
      hre.tasks.getTask("build").run({ noContracts: true, quiet: true })
    );
    testRootPaths = r.testRootPaths;
  } else {
    const r: any = await timed("build:all", () =>
      hre.tasks.getTask("build").run({ files: [], quiet: true })
    );
    testRootPaths = r.testRootPaths;
  }

  const scopes: Array<"contracts" | "tests"> = split
    ? ["contracts", "tests"]
    : ["contracts"];

  // Split the two halves of loadArtifacts(): reading + shaping the per-contract
  // artifacts (ABI/bytecode) vs reading the build-info outputs used for traces.
  const artifactManagers = [];
  for (const scope of scopes) {
    const dir = await hre.solidity.getArtifactsDirectory(scope);
    artifactManagers.push(new ArtifactManagerImplementation(dir));
  }

  const edrArtifactsWithMetadata = await timed(
    "artifacts:load",
    async () => {
      const out = [];
      for (const am of artifactManagers) {
        out.push(...(await buildEdrArtifactsWithMetadata(am)));
      }
      return out;
    },
    (r) => `${r.length} artifacts`
  );

  const allBuildInfosAndOutputs = await timed(
    "buildInfos:load",
    async () => {
      const out = [];
      for (const am of artifactManagers) {
        out.push(...(await getBuildInfosAndOutputs(am)));
      }
      return out;
    },
    (r) =>
      `${r.length} build infos, ${(
        r.reduce(
          (acc: number, b: any) =>
            acc + b.buildInfo.byteLength + b.output.byteLength,
          0
        ) /
        1024 /
        1024
      ).toFixed(1)} MiB`
  );

  const testRootPathsSet = new Set(testRootPaths);
  const testSuiteArtifacts = edrArtifactsWithMetadata
    .filter(({ userSourceName }: any) =>
      testRootPathsSet.has(
        resolveFromRoot(hre.config.paths.root, userSourceName)
      )
    )
    .filter(({ edrArtifact }: any) => isTestSuiteArtifact(edrArtifact));
  const testSuiteIds = testSuiteArtifacts.map(
    ({ edrArtifact }: any) => edrArtifact.id
  );

  const { getTestFunctionOverrides } = await loadInlineConfig();
  const testFunctionOverrides = await timed(
    "inlineConfig:collect",
    () => getTestFunctionOverrides(testSuiteArtifacts, allBuildInfosAndOutputs),
    (r: any) => `${Object.keys(r ?? {}).length} entries`
  );

  const testRunnerConfig = await timed("runnerConfig:build", () =>
    solidityTestConfigToSolidityTestRunnerConfigArgs({
      chainType: "l1",
      projectRoot: hre.config.paths.root,
      config: hre.config.test.solidity,
      verbosity: 0,
      observability: undefined,
      testPattern: ARGS.grep,
      generateGasReport: false,
      testFunctionOverrides,
    } as any)
  );

  const tracingConfig = {
    buildInfos: allBuildInfosAndOutputs.map(({ buildInfo, output }: any) => ({
      buildInfo,
      output,
    })),
    ignoreContracts: false,
  };

  const context = await timed("edr:context", async () => {
    const ctx = new EdrContext();
    await ctx.registerSolidityTestRunnerFactory(
      L1_CHAIN_TYPE,
      l1SolidityTestRunnerFactory()
    );
    return ctx;
  });

  const artifacts = edrArtifactsWithMetadata.map(
    ({ edrArtifact }: any) => edrArtifact
  );

  let ids = testSuiteIds;
  if (ARGS.grep !== undefined) {
    ids = ids.filter((id: any) =>
      `${id.source}:${id.name}`.includes(ARGS.grep!)
    );
  }

  const [, results] = await timed(
    "solidityTests:run",
    () =>
      runAllSolidityTests(
        context,
        L1_CHAIN_TYPE,
        artifacts,
        ids,
        tracingConfig as any,
        testRunnerConfig
      ),
    (r: any) => `${r[1].length} suites`
  );

  const totalMs = performance.now() - totalStart;

  // ------------------------------------------------------------ report

  let pass = 0;
  let fail = 0;
  let skip = 0;
  for (const suite of results as SuiteResult[]) {
    for (const t of suite.testResults) {
      if (t.status === TestStatus.Success) pass++;
      else if (t.status === TestStatus.Failure) fail++;
      else skip++;
    }
  }

  process.stderr.write(`\n=== breakdown (${ARGS.label}) ===\n`);
  const accounted = phases.reduce((a, p) => a + p.ms, 0);
  for (const p of phases) {
    process.stderr.write(
      `${p.name.padEnd(28)} ${p.ms.toFixed(1).padStart(10)} ms  ${(
        (100 * p.ms) /
        totalMs
      )
        .toFixed(1)
        .padStart(5)}%${p.note !== undefined ? `  ${p.note}` : ""}\n`
    );
  }
  process.stderr.write(
    `${"(unaccounted)".padEnd(28)} ${(totalMs - accounted)
      .toFixed(1)
      .padStart(10)} ms\n`
  );
  process.stderr.write(
    `${"TOTAL".padEnd(28)} ${totalMs.toFixed(1).padStart(10)} ms\n`
  );
  process.stderr.write(
    `\ntests: ${pass} passed, ${fail} failed, ${skip} skipped, ${results.length} suites\n`
  );

  // ------------------------------------------- inside solidityTests:run
  //
  // `solidityTests:run` is a single wall-clock number covering the whole native
  // run, and at fuzz.runs=1000 it is ~90% of the total -- so on its own it says
  // nothing about *what* is slow. EDR already reports durationNs per suite and
  // per test plus the test kind, so the interior can be broken down without any
  // extra instrumentation.
  const runPhaseMs =
    phases.find((p) => p.name === "solidityTests:run")?.ms ?? NaN;
  const breakdown = summariseRun(results as SuiteResult[], runPhaseMs);
  printRunBreakdown(breakdown, runPhaseMs);

  if (ARGS.json !== undefined) {
    fs.writeFileSync(
      ARGS.json,
      JSON.stringify(
        {
          label: ARGS.label,
          repo: ARGS.repo,
          fuzzRuns: ARGS.fuzzRuns,
          totalMs,
          phases,
          tests: { pass, fail, skip, suites: results.length },
          solidityTestsRun: breakdown,
        },
        null,
        2
      )
    );
  }
}

// ---------------------------------------------------------------- interior

const NS_PER_MS = 1e6;

type Kind = "standard" | "fuzz" | "invariant";

/**
 * Discriminate the test kind union. The three shapes are only distinguishable
 * structurally: InvariantTestKind has `calls`, FuzzTestKind has `runs` but not
 * `calls`, StandardTestKind has neither.
 */
function kindOf(kind: any): Kind {
  if (kind === null || kind === undefined) return "standard";
  if ("calls" in kind) return "invariant";
  if ("runs" in kind) return "fuzz";
  return "standard";
}

interface RunBreakdown {
  suiteWallMs: number;
  suiteDurationSumMs: number;
  testDurationSumMs: number;
  parallelism: number;
  maxSuiteMs: number;
  outsideLongestSuiteMs: number;
  byKind: Record<
    Kind,
    { count: number; ms: number; share: number; runs: number }
  >;
  topSuites: Array<{ name: string; ms: number; tests: number }>;
  topTests: Array<{ name: string; suite: string; ms: number; kind: Kind }>;
}

function summariseRun(
  suites: SuiteResult[],
  runPhaseMs: number
): RunBreakdown {
  const byKind: RunBreakdown["byKind"] = {
    standard: { count: 0, ms: 0, share: 0, runs: 0 },
    fuzz: { count: 0, ms: 0, share: 0, runs: 0 },
    invariant: { count: 0, ms: 0, share: 0, runs: 0 },
  };

  let suiteDurationSumMs = 0;
  let testDurationSumMs = 0;
  const suiteRows: RunBreakdown["topSuites"] = [];
  const testRows: RunBreakdown["topTests"] = [];

  for (const suite of suites) {
    const suiteMs = Number(suite.durationNs) / NS_PER_MS;
    suiteDurationSumMs += suiteMs;
    suiteRows.push({
      name: suite.id.name,
      ms: suiteMs,
      tests: suite.testResults.length,
    });

    for (const t of suite.testResults) {
      const ms = Number(t.durationNs) / NS_PER_MS;
      const k = kindOf(t.kind);
      testDurationSumMs += ms;
      byKind[k].count += 1;
      byKind[k].ms += ms;
      // `runs` is the fuzz/invariant iteration count -- how much work the case
      // actually did, which is what makes fuzz cases expensive.
      const runs = (t.kind as any)?.runs;
      if (runs !== undefined) byKind[k].runs += Number(runs);
      testRows.push({ name: t.name, suite: suite.id.name, ms, kind: k });
    }
  }

  for (const k of Object.keys(byKind) as Kind[]) {
    byKind[k].share =
      testDurationSumMs > 0 ? (100 * byKind[k].ms) / testDurationSumMs : 0;
  }

  suiteRows.sort((a, b) => b.ms - a.ms);
  testRows.sort((a, b) => b.ms - a.ms);

  return {
    suiteWallMs: runPhaseMs,
    suiteDurationSumMs,
    testDurationSumMs,
    // Suites run concurrently, so the summed suite time exceeds the wall clock.
    parallelism: runPhaseMs > 0 ? suiteDurationSumMs / runPhaseMs : NaN,
    // The longest single suite is the critical path: the run cannot finish
    // sooner than this no matter how many cores are available.
    maxSuiteMs: suiteRows.length > 0 ? suiteRows[0].ms : 0,
    // Everything the wall clock spent outside that critical-path suite. This is
    // a *mixture* -- one-shot native setup (artifact deserialization, library
    // linking, revert-decoder construction, trace identification, gas report)
    // plus scheduling and the tail of other suites -- so it is an upper bound on
    // one-shot overhead, not a measurement of it. Use the perf capture
    // (`analyze.py subsystems`) to separate those.
    outsideLongestSuiteMs:
      runPhaseMs - (suiteRows.length > 0 ? suiteRows[0].ms : 0),
    byKind,
    topSuites: suiteRows.slice(0, 10),
    topTests: testRows.slice(0, 10),
  };
}

function printRunBreakdown(b: RunBreakdown, runPhaseMs: number) {
  const w = process.stderr.write.bind(process.stderr);
  w(`\n=== inside solidityTests:run (${runPhaseMs.toFixed(1)} ms wall) ===\n`);
  w(
    `sum of suite durations      ${b.suiteDurationSumMs.toFixed(1).padStart(10)} ms   ` +
      `=> ~${b.parallelism.toFixed(2)}x parallelism across suites\n`
  );
  w(
    `sum of test durations       ${b.testDurationSumMs.toFixed(1).padStart(10)} ms   ` +
      `(${(b.suiteDurationSumMs - b.testDurationSumMs).toFixed(1)} ms in suites but outside tests: deploy + setUp)\n`
  );
  w(
    `longest single suite        ${b.maxSuiteMs.toFixed(1).padStart(10)} ms   ` +
      `critical path -- the run cannot beat this\n`
  );
  w(
    `outside the longest suite   ${b.outsideLongestSuiteMs.toFixed(1).padStart(10)} ms   ` +
      `one-shot native setup + scheduling + other suites' tail (upper bound, not a measurement)\n`
  );

  w(`\nby test kind (share of summed test time):\n`);
  for (const k of ["standard", "fuzz", "invariant"] as Kind[]) {
    const v = b.byKind[k];
    if (v.count === 0) continue;
    const runs = v.runs > 0 ? `, ${v.runs} iterations` : "";
    w(
      `  ${k.padEnd(10)} ${String(v.count).padStart(4)} tests  ${v.ms
        .toFixed(1)
        .padStart(9)} ms  ${v.share.toFixed(1).padStart(5)}%${runs}\n`
    );
  }

  w(`\nslowest suites:\n`);
  for (const s of b.topSuites) {
    w(
      `  ${s.ms.toFixed(1).padStart(9)} ms  ${String(s.tests).padStart(3)} tests  ${s.name}\n`
    );
  }

  w(`\nslowest individual tests:\n`);
  for (const t of b.topTests) {
    w(
      `  ${t.ms.toFixed(1).padStart(9)} ms  ${t.kind.padEnd(9)} ${t.suite}::${t.name}\n`
    );
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
