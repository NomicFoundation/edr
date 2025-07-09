import { assert } from "chai";

import { EdrContext, L1_CHAIN_TYPE, l1SolidityTestRunnerFactory } from "..";
import { loadContract, runAllSolidityTests } from "./helpers";

describe("Solidity Tests", () => {
  const context = new EdrContext();

  before(async () => {
    await context.registerSolidityTestRunnerFactory(
      L1_CHAIN_TYPE,
      l1SolidityTestRunnerFactory()
    );
  });

  it("executes basic tests", async function () {
    const artifacts = [
      loadContract("./data/artifacts/default/SetupConsistencyCheck.json"),
      loadContract("./data/artifacts/default/PaymentFailureTest.json"),
    ];
    // All artifacts are test suites.
    const testSuites = artifacts.map((artifact) => artifact.id);
    const config = {
      projectRoot: __dirname,
    };

    const results = await runAllSolidityTests(
      context,
      L1_CHAIN_TYPE,
      artifacts,
      testSuites,
      config
    );

    assert.equal(results.length, artifacts.length);

    for (const res of results) {
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

  it("throws errors", async function () {
    const artifacts = [
      loadContract("./data/artifacts/default/SetupConsistencyCheck.json"),
      loadContract("./data/artifacts/default/PaymentFailureTest.json"),
    ];
    // All artifacts are test suites.
    const testSuites = artifacts.map((artifact) => artifact.id);
    const config = {
      projectRoot: __dirname,
      // Memory limit is too large
      memoryLimit: 2n ** 65n,
    };

    await assert.isRejected(
      runAllSolidityTests(
        context,
        L1_CHAIN_TYPE,
        artifacts,
        testSuites,
        config
      ),
      Error
    );
  });

  it("error callback is called if contract bytecode is invalid", async function () {
    const artifacts = [
      loadContract("./data/artifacts/default/SetupConsistencyCheck.json"),
      loadContract("./data/artifacts/default/PaymentFailureTest.json"),
    ];
    // All artifacts are test suites.
    const testSuites = artifacts.map((artifact) => artifact.id);
    const config = {
      projectRoot: __dirname,
    };

    artifacts[0].contract.bytecode = "invalid bytecode";

    await assert.isRejected(
      runAllSolidityTests(
        context,
        L1_CHAIN_TYPE,
        artifacts,
        testSuites,
        config
      ),
      new RegExp("Hex decoding error")
    );
  });

  it("filters tests according to pattern", async function () {
    const artifacts = [
      loadContract("./data/artifacts/default/SetupConsistencyCheck.json"),
    ];
    // All artifacts are test suites.
    const testSuites = artifacts.map((artifact) => artifact.id);

    const results = await runAllSolidityTests(
      context,
      L1_CHAIN_TYPE,
      artifacts,
      testSuites,
      {
        projectRoot: __dirname,
        testPattern: "Multiply",
      }
    );

    assert.equal(results.length, artifacts.length);

    for (const res of results) {
      if (res.id.name.includes("SetupConsistencyCheck")) {
        assert.equal(res.testResults.length, 1);
        assert.equal(res.testResults[0].name, "testMultiply()");
      } else {
        assert.fail("Unexpected test suite name: " + res.id.name);
      }
    }
  });
});
