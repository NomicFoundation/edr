import { assert } from "chai";

import {
  ArtifactId,
  ContractData,
  SuiteResult,
  runSolidityTests,
  Artifact,
} from "..";

describe("Solidity Tests", () => {
  it("executes basic tests", async function () {
    const artifacts = [
      loadContract("./artifacts/SetupConsistencyCheck.json"),
      loadContract("./artifacts/PaymentFailureTest.json"),
    ];

    // All artifacts are test suites.
    const testSuites = artifacts.map((artifact) => artifact.id);

    const results: Array<SuiteResult> = await new Promise((resolve) => {
      const gasReport = false;
      const resultsFromCallback: Array<SuiteResult> = [];

      runSolidityTests(
        artifacts,
        testSuites,
        gasReport,
        (result: SuiteResult) => {
          resultsFromCallback.push(result);
          if (resultsFromCallback.length === artifacts.length) {
            resolve(resultsFromCallback);
          }
        },
      );
    });

    assert.equal(results.length, artifacts.length);

    for (let res of results) {
      if (res.id.name.includes("SetupConsistencyCheck")) {
        assert.equal(res.testResults.length, 2);
        assert.equal(res.testResults[0].status, "Success");
        assert.equal(res.testResults[1].status, "Success");
      } else if (res.id.name.includes("PaymentFailureTest")) {
        assert.equal(res.testResults.length, 1);
        assert.equal(res.testResults[0].status, "Failure");
      } else {
        assert.fail("Unexpected test suite name: " + res.id.name);
      }
    }
  });
});

// Load a contract built with Hardhat into a test suite
function loadContract(artifactPath: string): Artifact {
  const compiledContract = require(artifactPath);

  const id: ArtifactId = {
    name: compiledContract.contractName,
    solcVersion: "0.8.18",
    source: compiledContract.sourceName,
  };

  const contract: ContractData = {
    abi: JSON.stringify(compiledContract.abi),
    bytecode: compiledContract.bytecode,
  };

  return {
    id,
    contract,
  };
}
