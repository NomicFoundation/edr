import {
  ArtifactId,
  SuiteResult,
  runSolidityTests,
  Artifact,
  SolidityTestRunnerConfigArgs,
  TestResult,
} from "@nomicfoundation/edr";

/**
 * Run all the given solidity tests and returns the whole results after finishing.
 */
export async function runAllSolidityTests(
  artifacts: Artifact[],
  testSuites: ArtifactId[],
  configArgs: SolidityTestRunnerConfigArgs,
  testResultCallback: (
    suiteResult: SuiteResult,
    testResult: TestResult
  ) => void = () => {}
): Promise<SuiteResult[]> {
  return new Promise((resolve, reject) => {
    const resultsFromCallback: SuiteResult[] = [];

    runSolidityTests(
      artifacts,
      testSuites,
      configArgs,
      (suiteResult: SuiteResult) => {
        for (const testResult of suiteResult.testResults) {
          testResultCallback(suiteResult, testResult);
        }

        resultsFromCallback.push(suiteResult);
        if (resultsFromCallback.length === artifacts.length) {
          resolve(resultsFromCallback);
        }
      },
      reject
    );
  });
}
