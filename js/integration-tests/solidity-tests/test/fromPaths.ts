import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { before, describe, it } from "node:test";
import {
  L1_CHAIN_TYPE,
  SolidityTestResult,
  SolidityTestRunnerConfigArgs,
  SuiteResult,
  TestSuiteReference,
  TestStatus,
} from "@nomicfoundation/edr";
import { runAllSolidityTests } from "@nomicfoundation/edr-helpers";
import { assertStackTraces, TestContext } from "./testContext.js";

const ARTIFACTS_DIR = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "artifacts"
);

/**
 * Runs test suites through `runSolidityTestsFromPaths`, resolving once both
 * the run has completed and every suite callback has fired.
 */
function runAllSolidityTestsFromPaths(
  testContext: TestContext,
  testSuites: TestSuiteReference[],
  config?: Partial<SolidityTestRunnerConfigArgs>
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

    testContext.edrContext
      .runSolidityTestsFromPaths(
        L1_CHAIN_TYPE,
        [ARTIFACTS_DIR],
        testSuites,
        {
          ...testContext.defaultConfig(),
          ...config,
        },
        (suiteResult: SuiteResult) => {
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

/** Maps `<suite id source>:<suite id name>:<test name>` to its status. */
function testStatuses(suiteResults: SuiteResult[]): Map<string, TestStatus> {
  const statuses = new Map<string, TestStatus>();
  for (const suiteResult of suiteResults) {
    for (const testResult of suiteResult.testResults) {
      statuses.set(
        `${suiteResult.id.source}:${suiteResult.id.name}:${testResult.name}`,
        testResult.status
      );
    }
  }
  return statuses;
}

describe("runSolidityTestsFromPaths", () => {
  let testContext: TestContext;

  before(async () => {
    testContext = await TestContext.setup();
  });

  it("produces the same results as runSolidityTests", async function () {
    // `LinkingTest` additionally exercises library linking of the
    // disk-loaded artifacts.
    const suiteNames = new Set(["SuccessAndFailureTest", "LinkingTest"]);
    const testSuiteIds = testContext.matchingTests(suiteNames);
    assert.equal(testSuiteIds.length, 2);

    const [, suiteResultsFromArtifacts] = await runAllSolidityTests(
      testContext.edrContext,
      L1_CHAIN_TYPE,
      testContext.artifacts,
      testSuiteIds,
      testContext.tracingConfig,
      testContext.defaultConfig()
    );

    const [, suiteResultsFromPaths] = await runAllSolidityTestsFromPaths(
      testContext,
      testSuiteIds.map((id) => ({ source: id.source, name: id.name }))
    );

    assert.deepEqual(
      testStatuses(suiteResultsFromPaths),
      testStatuses(suiteResultsFromArtifacts)
    );
  });

  it("resolves test suites by user-facing source name", async function () {
    const [, suiteResults] = await runAllSolidityTestsFromPaths(testContext, [
      {
        source: "test-contracts/SuccessAndFailure.t.sol",
        name: "SuccessAndFailureTest",
      },
    ]);

    assert.equal(suiteResults.length, 1);
    assert.equal(suiteResults[0].testResults.length, 2);
    assert.equal(
      suiteResults[0].testResults.filter(
        (testResult) => testResult.status === TestStatus.Failure
      ).length,
      1
    );
  });

  it("generates stack traces from disk-loaded build infos", async function () {
    const [, suiteResults] = await runAllSolidityTestsFromPaths(testContext, [
      {
        source: "project/test-contracts/SuccessAndFailure.t.sol",
        name: "SuccessAndFailureTest",
      },
    ]);

    const failedTest = suiteResults
      .flatMap((suiteResult) => suiteResult.testResults)
      .find((testResult) => testResult.status === TestStatus.Failure);
    assert.ok(failedTest !== undefined, "expected a failing test");

    assertStackTraces(
      {
        stackTrace: failedTest.stackTrace() ?? undefined,
        reason: failedTest.reason ?? undefined,
      },
      "1 is not equal to 2",
      [{ contract: "SuccessAndFailureTest", function: "testThatFails" }]
    );
  });

  it("rejects unknown test suites", async function () {
    await assert.rejects(
      runAllSolidityTestsFromPaths(testContext, [
        { source: "test-contracts/SuccessAndFailure.t.sol", name: "Missing" },
      ]),
      /Unknown test suite contract: test-contracts\/SuccessAndFailure.t.sol:Missing/
    );
  });
});
