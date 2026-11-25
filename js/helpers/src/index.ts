import type { HardhatRuntimeEnvironment } from "hardhat/types/hre";
import * as os from "node:os";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { MultiProcessMutex } from "@nomicfoundation/hardhat-utils/synchronization";
import { coerce, lt } from "semver";
import {
  Artifact,
  ArtifactId,
  EdrContext,
  SuiteResult,
  SolidityTestRunnerConfigArgs,
  TestResult,
  TracingConfigWithBuffers,
  SolidityTestResult,
  BuildInfoAndOutput,
} from "@nomicfoundation/edr";

export type * from "hardhat/hre";

import { resolveFromRoot } from "@nomicfoundation/hardhat-utils/path";
import {
  buildEdrArtifactsWithMetadata,
  getBuildInfosAndOutputs,
  EdrArtifactWithMetadata,
} from "hardhat/internal/builtin-plugins/solidity-test/edr-artifacts";
import {
  isTestSuiteArtifact,
  warnDeprecatedTestFail,
} from "hardhat/internal/builtin-plugins/solidity-test/helpers";
import { ArtifactManagerImplementation } from "hardhat/internal/builtin-plugins/artifacts/artifact-manager";

let BUILD_MUTEX: MultiProcessMutex | undefined;

/** The oldest solc version whose sources EDR can parse. Collecting the
 * inline test configuration and the EIP-712 struct definitions requires
 * parsing, and EDR exempts no source from it. */
const OLDEST_PARSABLE_SOLC_VERSION = "0.8.0";

/** One test suite's solc source name, its user-facing source name (for a
 * warning), its path on disk, and the solc version its artifact was
 * compiled with. */
export interface TestSourceSelection {
  source: string;
  userSourceName: string;
  path: string;
  solcVersion: string;
}

/**
 * Decides which test source paths to hand EDR.
 *
 * EDR requires a non-empty map to name every selected source, and rejects
 * the whole run when one of them is older than
 * `OLDEST_PARSABLE_SOLC_VERSION`. Collection is therefore all or nothing: a
 * single unparsable source empties the map for the whole run, and its
 * user-facing name is returned so the caller can warn about it with
 * `warnUnparsableTestSources`.
 */
export function selectTestSourcePaths(sources: TestSourceSelection[]): {
  testSourcePaths: Record<string, string>;
  unparsableSources: string[];
} {
  const unparsableSources = Array.from(
    new Set(
      sources
        .filter(({ solcVersion }) =>
          lt(coerce(solcVersion) ?? solcVersion, OLDEST_PARSABLE_SOLC_VERSION)
        )
        .map(({ userSourceName }) => userSourceName)
    )
  ).sort();

  if (unparsableSources.length > 0) {
    return { testSourcePaths: {}, unparsableSources };
  }

  return {
    testSourcePaths: Object.fromEntries(
      sources.map(({ source, path: sourcePath }) => [source, sourcePath])
    ),
    unparsableSources,
  };
}

/**
 * Warns that the named test sources disable collection for the whole run.
 *
 * Call this only at the site that actually submits a run to EDR: a caller
 * that calls `buildSolidityTestsInput` once and reuses its selection across
 * several smaller EDR calls (as the integration-test harness does) would
 * otherwise print one warning per reuse, naming sources that may not even be
 * part of that particular call.
 */
export function warnUnparsableTestSources(userSourceNames: string[]): void {
  console.warn(
    `Warning: these test sources were compiled with a Solidity version ` +
      `older than ${OLDEST_PARSABLE_SOLC_VERSION}, which cannot be parsed: ` +
      `${userSourceNames.join(", ")}. Inline test configuration and the ` +
      `EIP-712 cheatcodes are disabled for this whole run.`
  );
}

/**
 * Run all the given solidity tests and returns the whole results after finishing.
 */
export async function runAllSolidityTests(
  context: EdrContext,
  chainType: string,
  artifacts: Artifact[],
  testSuites: ArtifactId[],
  tracingConfig: TracingConfigWithBuffers,
  configArgs: SolidityTestRunnerConfigArgs,
  testResultCallback: (
    suiteResult: SuiteResult,
    testResult: TestResult
  ) => void = () => {}
): Promise<[SolidityTestResult, SuiteResult[]]> {
  return new Promise((resolve, reject) => {
    const resultsFromCallback: SuiteResult[] = [];
    let solidityTestResult: SolidityTestResult | undefined;
    let isTestComplete = false;

    const tryResolve = () => {
      if (isTestComplete && resultsFromCallback.length === testSuites.length) {
        resolve([solidityTestResult!, resultsFromCallback]);
      }
    };

    context
      .runSolidityTests(
        chainType,
        artifacts,
        testSuites,
        configArgs,
        tracingConfig,
        (suiteResult: SuiteResult) => {
          for (const testResult of suiteResult.testResults) {
            testResultCallback(suiteResult, testResult);
          }
          resultsFromCallback.push(suiteResult);
          tryResolve();
        }
      )
      .then((result) => {
        solidityTestResult = result;
        isTestComplete = true;
        tryResolve();
      })
      .catch(reject);
  });
}

/*
 Build Solidity tests in a Hardhat v3 project.
 Based on https://github.com/NomicFoundation/hardhat/blob/6e9903439fcf145e4b603be60c4c1c036095a241/v-next/hardhat/src/internal/builtin-plugins/solidity-test/task-action.ts
 */
export async function buildSolidityTestsInput(
  hre: HardhatRuntimeEnvironment
): Promise<{
  artifacts: Artifact[];
  testSuiteIds: ArtifactId[];
  tracingConfig: TracingConfigWithBuffers;
  /** Maps each test suite's solc source name to its absolute path on disk.
   * Used by EDR to parse inline test configuration and EIP-712 struct
   * definitions from the sources.
   *
   * Empty when a selected suite was compiled with a solc version older than
   * `OLDEST_PARSABLE_SOLC_VERSION`, which disables collection for the whole
   * run, or when no suite is selected at all. */
  testSourcePaths: Record<string, string>;
  /** The selection input behind `testSourcePaths`, one entry per selected
   * suite, before the all-or-nothing decision collapses it. A caller that
   * submits only part of `testSuiteIds` to EDR in a given run can pass the
   * matching subset of these through `selectTestSourcePaths` to get that
   * run's own map, instead of inheriting a decision made for suites it did
   * not include. */
  testSourceSelections: TestSourceSelection[];
  /** The user-facing names of the suites that emptied `testSourcePaths`,
   * sorted and deduplicated; empty when `testSourcePaths` maps every suite.
   * Not warned about here: only the site that actually submits `testSuiteIds`
   * to EDR knows whether this decision applies to its run, so it calls
   * `warnUnparsableTestSources` itself when this is non-empty. */
  unparsableSources: string[];
}> {
  let testRootPaths: string[];

  // Cache assumes one build process at a time.
  await buildMutex().use(async () => {
    await hre.tasks.getTask("build").run({
      noTests: true,
      quiet: true,
    });
    // Run the build task for test files
    const result: { testRootPaths: string[] } = await hre.tasks
      .getTask("build")
      .run({
        // If no specific files are passed, it means compile all source files
        files: [],
        noContracts: true,
        quiet: true,
      });
    testRootPaths = result.testRootPaths;
  });

  // EDR needs all artifacts (contracts + tests). In unified mode (the default
  // since Hardhat 3.4.x), both scopes point to the same directory, so iterating
  // both would load every artifact twice.
  const scopes = hre.config.solidity.splitTestsCompilation
    ? (["contracts", "tests"] as const)
    : (["contracts"] as const);
  const edrArtifacts: EdrArtifactWithMetadata[] = [];
  const buildInfos: BuildInfoAndOutput[] = [];
  for (const scope of scopes) {
    const artifactsDir = await hre.solidity.getArtifactsDirectory(scope);
    const artifactManager = new ArtifactManagerImplementation(artifactsDir);
    edrArtifacts.push(
      ...(await buildEdrArtifactsWithMetadata(artifactManager))
    );
    buildInfos.push(...(await getBuildInfosAndOutputs(artifactManager)));
  }

  // Duplicate artifacts break `runAllSolidityTests`: the native runner dedupes
  // suites by ArtifactId, so it fires fewer callbacks than `testSuites.length`
  // and the promise never resolves. Warn loudly if Hardhat ever feeds us
  // duplicates so the failure is visible instead of a silent hang.
  const seenIds = new Set<string>();
  const duplicateIds: string[] = [];
  for (const { edrArtifact } of edrArtifacts) {
    const key = `${edrArtifact.id.source}:${edrArtifact.id.name}@${edrArtifact.id.solcVersion}`;
    if (seenIds.has(key)) {
      duplicateIds.push(key);
    } else {
      seenIds.add(key);
    }
  }
  if (duplicateIds.length > 0) {
    console.warn(
      `Warning: buildSolidityTestsInput loaded ${duplicateIds.length} duplicate artifact(s); ` +
        `runAllSolidityTests will hang because the native runner dedupes suites. ` +
        `Duplicates: ${duplicateIds.join(", ")}`
    );
  }

  const sourceNameToUserSourceName = new Map(
    edrArtifacts.map(({ userSourceName, edrArtifact }) => [
      edrArtifact.id.source,
      userSourceName,
    ])
  );

  edrArtifacts.forEach(({ userSourceName, edrArtifact }) => {
    if (
      testRootPaths.includes(
        resolveFromRoot(hre.config.paths.root, userSourceName)
      ) &&
      isTestSuiteArtifact(edrArtifact)
    ) {
      warnDeprecatedTestFail(edrArtifact, sourceNameToUserSourceName);
    }
  });

  const testSuites = edrArtifacts
    .filter(({ userSourceName }) =>
      testRootPaths.includes(
        resolveFromRoot(hre.config.paths.root, userSourceName)
      )
    )
    .filter(({ edrArtifact }) => isTestSuiteArtifact(edrArtifact));

  const testSuiteIds = testSuites.map(({ edrArtifact }) => edrArtifact.id);

  const testSourceSelections: TestSourceSelection[] = testSuites.map(
    ({ userSourceName, edrArtifact }) => ({
      source: edrArtifact.id.source,
      userSourceName,
      path: resolveFromRoot(hre.config.paths.root, userSourceName),
      solcVersion: edrArtifact.id.solcVersion,
    })
  );

  const { testSourcePaths, unparsableSources } =
    selectTestSourcePaths(testSourceSelections);

  const artifacts = edrArtifacts.map(({ edrArtifact }) => edrArtifact);

  const tracingConfig: TracingConfigWithBuffers = {
    buildInfos,
    ignoreContracts: false,
  };

  // Keep `importMappings` out of this input. EDR matches import paths exactly
  // as written, while Hardhat's source names are prefixed (`project/…`,
  // `npm/pkg@version/…`). A map keyed by source name would therefore match no
  // real import.
  //
  // The cost is that structs behind a package import are unreachable, so
  // `vm.eip712HashType` reports them as unknown types. Closing that gap needs
  // Hardhat's resolver to supply entries keyed by the import path as written.
  return {
    artifacts,
    testSuiteIds,
    tracingConfig,
    testSourcePaths,
    testSourceSelections,
    unparsableSources,
  };
}

/* Get the directory name of the current file based on the `import.meta.url`*/
export function dirName(importUrl: string) {
  const fileName = fileURLToPath(importUrl);
  return path.dirname(fileName);
}

function buildMutex() {
  if (BUILD_MUTEX === undefined) {
    // hardhat-utils ≥4 takes the lock file's absolute path; this is where
    // the name-based ≤3 API put it.
    BUILD_MUTEX = new MultiProcessMutex(
      path.join(os.tmpdir(), "edr-helpers-build-mutex.txt")
    );
  }
  return BUILD_MUTEX;
}
