import chai, { assert } from "chai";
import chaiAsPromised from "chai-as-promised";

import { SolidityTestRunner, TestSuite, TestContract, ArtifactId, SuiteResult } from "..";

describe("Solidity Tests", () => {
  it("executes basic tests", async function() {
    const testSuites = [loadContract("./SetupConsistencyCheck.json"), loadContract("./PaymentFailureTest.json")];

    const resultsFromCallback: Array<SuiteResult> = [];
    const runner = new SolidityTestRunner((...args) => {
      resultsFromCallback.push(args[1] as SuiteResult);
    });

    const result = await runner.runTests(testSuites);
    assert.equal(resultsFromCallback.length, testSuites.length);
    assert.equal(resultsFromCallback.length, result.length);

    for (let res of result) {
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
    source: compiledContract.sourceName
  };

  const testContract: TestContract = {
    abi: JSON.stringify(compiledContract.abi),
    bytecode: compiledContract.bytecode,
    libsToDeploy: [],
    libraries: []
  };

  return {
    id: artifactId,
    contract: testContract
  };
}
