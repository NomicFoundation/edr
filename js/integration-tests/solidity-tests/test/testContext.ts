import {
  Artifact,
  ArtifactId,
  FuzzConfigArgs,
  InvariantConfigArgs,
  type SolidityTestRunnerConfigArgs,
} from "@ignored/edr";
import {
  buildSolidityTestsInput,
  runAllSolidityTests,
} from "@nomicfoundation/edr-helpers";
import hre from "hardhat";
import { TracingConfig } from "hardhat/internal/hardhat-network/provider/node-types";
import { SolidityStackTrace } from "hardhat/internal/hardhat-network/stack-traces/solidity-stack-trace";

export class TestContext {
  readonly rpcUrl = process.env.ALCHEMY_URL;
  readonly rpcCachePath: string = "./edr-cache";
  readonly fuzzFailuresPersistDir: string = "./edr-cache/fuzz";
  readonly invariantFailuresPersistDir: string = "./edr-cache/invariant";
  readonly artifacts: Artifact[];
  readonly testSuiteIds: ArtifactId[];
  readonly tracingConfig: TracingConfig;

  private constructor(
    artifacts: Artifact[],
    testSuiteIds: ArtifactId[],
    tracingConfig: TracingConfig
  ) {
    this.artifacts = artifacts;
    this.testSuiteIds = testSuiteIds;
    this.tracingConfig = tracingConfig;
  }

  static async setup(): Promise<TestContext> {
    const results = await buildSolidityTestsInput(hre.artifacts);
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
      trace: true,
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

    const stackTraces = new Map<string, SolidityStackTrace>();
    for (const suiteResult of suiteResults) {
      for (const testResult of suiteResult.testResults) {
        let failed = testResult.status === "Failure";
        totalTests++;
        if (failed) {
          failedTests++;
          if (testResult.stackTrace !== undefined) {
            stackTraces.set(testResult.name, testResult.stackTrace);
          }
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
  stackTraces: Map<string, SolidityStackTrace>;
}
