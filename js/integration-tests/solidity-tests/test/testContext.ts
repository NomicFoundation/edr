import {
  Artifact,
  ArtifactId,
  HeuristicFailed,
  type SolidityTestRunnerConfigArgs,
  StackTrace,
  TracingConfigWithBuffers,
  UnexpectedError,
  UnsafeToReplay,
} from "@ignored/edr";
import {
  buildSolidityTestsInput,
  runAllSolidityTests,
} from "@nomicfoundation/edr-helpers";
import hre from "hardhat";
import assert from "node:assert/strict";

type StackTraceResult =
  | StackTrace
  | UnexpectedError
  | HeuristicFailed
  | UnsafeToReplay
  | null
  | undefined;

export class TestContext {
  readonly rpcUrl = process.env.ALCHEMY_URL;
  readonly rpcCachePath: string = "./edr-cache";
  readonly fuzzFailuresPersistDir: string = "./edr-cache/fuzz";
  readonly invariantFailuresPersistDir: string = "./edr-cache/invariant";
  readonly artifacts: Artifact[];
  readonly testSuiteIds: ArtifactId[];
  readonly tracingConfig: TracingConfigWithBuffers;

  private constructor(
    artifacts: Artifact[],
    testSuiteIds: ArtifactId[],
    tracingConfig: TracingConfigWithBuffers
  ) {
    this.artifacts = artifacts;
    this.testSuiteIds = testSuiteIds;
    this.tracingConfig = tracingConfig;
  }

  static async setup(): Promise<TestContext> {
    const results = await buildSolidityTestsInput(hre);
    return new TestContext(
      results.artifacts,
      results.testSuiteIds,
      results.tracingConfig
    );
  }

  defaultConfig(): SolidityTestRunnerConfigArgs {
    return {
      projectRoot: hre.config.paths.root,
      rpcCachePath: this.rpcCachePath,
    };
  }

  async runTestsWithStats(
    contractName: string,
    config?: Omit<SolidityTestRunnerConfigArgs, "projectRoot">
  ): Promise<SolidityTestsRunResult> {
    let totalTests = 0;
    let failedTests = 0;

    let testContracts = this.matchingTest(contractName);
    if (testContracts.length === 0) {
      throw new Error(`No matching test contract found for ${contractName}`);
    }

    const suiteResults = await runAllSolidityTests(
      this.artifacts,
      testContracts,
      this.tracingConfig,
      {
        ...this.defaultConfig(),
        ...config,
      }
    );

    const stackTraces = new Map();
    for (const suiteResult of suiteResults) {
      for (const testResult of suiteResult.testResults) {
        let failed = testResult.status === "Failure";
        totalTests++;
        if (failed) {
          failedTests++;
          const stackTrace = testResult.stackTrace();
          stackTraces.set(testResult.name, {
            stackTrace,
            reason: testResult.reason,
          });
        }
      }
    }
    return { totalTests, failedTests, stackTraces };
  }

  matchingTest(contractName: string): ArtifactId[] {
    return this.matchingTests(new Set([contractName]));
  }

  matchingTests(testContractNames: Set<string>): ArtifactId[] {
    return this.testSuiteIds.filter((testSuiteId) => {
      return testContractNames.has(testSuiteId.name);
    });
  }
}

interface SolidityTestsRunResult {
  totalTests: number;
  failedTests: number;
  stackTraces: Map<
    string,
    {
      stackTrace: StackTraceResult | undefined;
      reason: string | undefined;
    }
  >;
}

type ActualStackTraceResult =
  | { stackTrace: StackTraceResult | undefined; reason: string | undefined }
  | undefined
  | null;

export function assertStackTraces(
  actual: ActualStackTraceResult,
  expectedReason: string,
  expectedEntries: {
    function: string;
    contract: string;
  }[]
) {
  if (
    actual === undefined ||
    actual == null ||
    actual.stackTrace === undefined ||
    actual.stackTrace === null
  ) {
    throw new Error("Stack trace is undefined");
  }
  if (actual.stackTrace.kind === "HeuristicFailed") {
    throw new Error("Stack trace result is 'HeuristicFailed'");
  }
  if (actual.stackTrace.kind === "UnsafeToReplay") {
    throw new Error(
      `Stack trace is unsafe to replay. Global forking with latest block: '${actual.stackTrace.impureCheatcodes}' to impure cheatcodes: '${actual.stackTrace.impureCheatcodes}'`
    );
  }
  if (actual.stackTrace.kind === "UnexpectedError") {
    throw new Error(
      `Unexpected stack trace error: '${actual.stackTrace.errorMessage}'`
    );
  }

  const stackTrace = actual.stackTrace;
  if (stackTrace === undefined) {
    throw new Error("Stack trace is missing");
  }
  assert.equal(stackTrace.entries.length, expectedEntries.length);
  for (let i = 0; i < stackTrace.entries.length; i += 1) {
    const expected = expectedEntries[i];
    const sourceReference = stackTrace.entries[i].sourceReference;
    if (sourceReference === undefined) {
      throw new Error(
        `source reference is undefined for contract '${expected.contract}' function '${expected.function}'`
      );
    }
    assert.equal(sourceReference.contract, expected.contract);
    assert.equal(sourceReference.function, expected.function);
    assert(sourceReference.sourceContent.includes(expected.function));
  }
}

export function assertImpureCheatcode(
  actual: ActualStackTraceResult,
  expectedCheatcode: string
) {
  if (
    actual === undefined ||
    actual === null ||
    actual.stackTrace?.kind !== "UnsafeToReplay"
  ) {
    throw new Error(
      `Expected unsafe to replay stack trace, instead it is: ${actual}`
    );
  }
  // When using forking from latest block, no stack trace should be generated as re-execution is unsafe.
  assert.equal(
    actual.stackTrace.impureCheatcodes.filter((cheatcode) =>
      cheatcode.includes(expectedCheatcode)
    ).length,
    1
  );
}
