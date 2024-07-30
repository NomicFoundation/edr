import { assert } from "chai";

import {
  TestSuite,
  TestContract,
  ArtifactId,
  SuiteResult,
  runSolidityTests,
} from "..";

describe("Solidity Tests", () => {
  it("executes basic tests", async function () {
    const testSuites = [
      loadContract("./artifacts/SetupConsistencyCheck.json"),
      loadContract("./artifacts/PaymentFailureTest.json"),
    ];

    const results: Array<SuiteResult> = await new Promise((resolve) => {
      const gasReport = false;
      const resultsFromCallback: Array<SuiteResult> = [];

      runSolidityTests(testSuites, gasReport, (result: SuiteResult) => {
        resultsFromCallback.push(result);
        if (resultsFromCallback.length === testSuites.length) {
          resolve(resultsFromCallback);
        }
      });
    });

    assert.equal(results.length, testSuites.length);

    for (let res of results) {
      if (res.name.includes("SetupConsistencyCheck")) {
        assert.equal(res.testResults.length, 2);
        assert.equal(res.testResults[0].status, "Success");
        assert.equal(res.testResults[1].status, "Success");
      } else if (res.name.includes("PaymentFailureTest")) {
        assert.equal(res.testResults.length, 1);
        assert.equal(res.testResults[0].status, "Failure");
      } else {
        assert.fail("Unexpected test suite name: " + res.name);
      }
    }
  });
});

// Load a contract built with Hardhat into a test suite
function loadContract(artifactPath: string): TestSuite {
  const compiledContract = require(artifactPath);

  const artifactId: ArtifactId = {
    // Artifact cache path is ignored in this test
    artifactCachePath: "./none",
    name: compiledContract.contractName,
    solcVersion: "0.8.18",
    source: compiledContract.sourceName,
  };

  const testContract: TestContract = {
    abi: JSON.stringify(compiledContract.abi),
    bytecode: compiledContract.bytecode,
    libsToDeploy: [],
    libraries: [],
  };

  return {
    id: artifactId,
    contract: testContract,
  };
}
