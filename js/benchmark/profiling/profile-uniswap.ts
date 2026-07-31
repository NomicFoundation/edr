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
        },
        null,
        2
      )
    );
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
