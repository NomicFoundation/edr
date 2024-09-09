import {
  Artifact,
  ArtifactId,
  FuzzConfigArgs,
  InvariantConfigArgs,
  type SolidityTestRunnerConfigArgs,
} from "@nomicfoundation/edr";
import {
  buildSolidityTestsInput,
  runAllSolidityTests,
} from "@nomicfoundation/edr-helpers";
import hre from "hardhat";

export class TestContext {
  readonly rpcUrl = process.env.ALCHEMY_URL;
  readonly rpcCachePath: string = "./edr-cache";
  readonly fuzzFailuresPersistDir: string = "./edr-cache/fuzz";
  readonly invariantFailuresPersistDir: string = "./edr-cache/invariant";
  readonly artifacts: Artifact[];
  readonly testSuiteIds: ArtifactId[];

  constructor(artifacts: Artifact[], testSuiteIds: ArtifactId[]) {
    this.artifacts = artifacts;
    this.testSuiteIds = testSuiteIds;
  }

  static async setup(): Promise<TestContext> {
    const results = await buildSolidityTestsInput(hre.artifacts);
    return new TestContext(results.artifacts, results.testSuiteIds);
  }

  defaultConfig(): SolidityTestRunnerConfigArgs {
    return {
      projectRoot: hre.config.paths.root,
      rpcCachePath: this.rpcCachePath,
    };
  }

  fuzzConfig(
    config?: Omit<FuzzConfigArgs, "failurePersistDir">
  ): FuzzConfigArgs {
    return {
      failurePersistDir: this.fuzzFailuresPersistDir,
      ...config,
    };
  }

  invariantConfig(
    config?: Omit<InvariantConfigArgs, "failurePersistDir">
  ): InvariantConfigArgs {
    return {
      failurePersistDir: this.invariantFailuresPersistDir,
      ...config,
    };
  }

  async runTestsWithStats(
    contractName: string,
    config?: Omit<SolidityTestRunnerConfigArgs, "projectRoot">
  ): Promise<SolidityTestsRunResult> {
    let totalTests = 0;
    let failedTests = 0;

    const suiteResults = await runAllSolidityTests(
      this.artifacts,
      this.matchingTest(contractName),
      {
        ...this.defaultConfig(),
        ...config,
      }
    );

    for (const suiteResult of suiteResults) {
      for (const testResult of suiteResult.testResults) {
        let failed = testResult.status === "Failure";
        totalTests++;
        if (failed) {
          failedTests++;
        }
      }
    }
    return { totalTests, failedTests };
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
}
