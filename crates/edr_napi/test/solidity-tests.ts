import { assert } from "chai";
import * as path from "path";

import {
  EdrContext,
  L1_CHAIN_TYPE,
  l1HardforkLatest,
  l1HardforkToString,
  l1SolidityTestRunnerFactory,
} from "..";
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
      disableTransactionGasCap: true,
      projectRoot: __dirname,
      hardfork: l1HardforkToString(l1HardforkLatest()),
    };

    const [, results] = await runAllSolidityTests(
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
      hardfork: l1HardforkToString(l1HardforkLatest()),
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
      hardfork: l1HardforkToString(l1HardforkLatest()),
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

  // The EIP-712 type cheatcodes resolve type names lazily by parsing the
  // running test contract's Solidity sources. These fixtures live on disk under
  // `data/contracts` so the provider can read them at `projectRoot`-relative
  // source paths (projectRoot is `__dirname`).
  const eip712ImportMappings = {
    "@fixtures/Eip712External.sol": path.join(
      __dirname,
      "data/contracts/external/Eip712External.sol"
    ),
  };

  it("resolves eip712 types lazily from the test contract's sources", async function () {
    const artifacts = [
      loadContract("./data/artifacts/default/Eip712LazyTest.json"),
    ];
    const testSuites = artifacts.map((artifact) => artifact.id);

    const [, results] = await runAllSolidityTests(
      context,
      L1_CHAIN_TYPE,
      artifacts,
      testSuites,
      {
        disableTransactionGasCap: true,
        projectRoot: __dirname,
        hardfork: l1HardforkToString(l1HardforkLatest()),
        eip712ImportMappings,
      }
    );

    assert.equal(results.length, 1);
    const suite = results[0];
    assert.isAbove(suite.testResults.length, 0);
    for (const res of suite.testResults) {
      assert.equal(
        res.status,
        "Success",
        `${res.name} failed: ${JSON.stringify(res.reason)}`
      );
    }
  });

  it("fails when an eip712 type cannot be resolved from sources", async function () {
    const artifacts = [
      loadContract("./data/artifacts/default/Eip712UnknownTest.json"),
    ];
    const testSuites = artifacts.map((artifact) => artifact.id);

    const [, results] = await runAllSolidityTests(
      context,
      L1_CHAIN_TYPE,
      artifacts,
      testSuites,
      {
        disableTransactionGasCap: true,
        projectRoot: __dirname,
        hardfork: l1HardforkToString(l1HardforkLatest()),
      }
    );

    assert.equal(results.length, 1);
    const suite = results[0];
    assert.equal(suite.testResults.length, 1);
    assert.equal(suite.testResults[0].status, "Failure");
  });

  it("resolves eip712 types across multiple suites in one run", async function () {
    // Exercises the shared provider serving two different root sources within
    // a single test run (parses run in parallel behind one service).
    const artifacts = [
      loadContract("./data/artifacts/default/Eip712LazyTest.json"),
      loadContract("./data/artifacts/default/Eip712UnknownTest.json"),
    ];
    const testSuites = artifacts.map((artifact) => artifact.id);

    const [, results] = await runAllSolidityTests(
      context,
      L1_CHAIN_TYPE,
      artifacts,
      testSuites,
      {
        disableTransactionGasCap: true,
        projectRoot: __dirname,
        hardfork: l1HardforkToString(l1HardforkLatest()),
        eip712ImportMappings,
      }
    );

    assert.equal(results.length, 2);
    for (const suite of results) {
      if (suite.id.name.includes("Eip712LazyTest")) {
        for (const res of suite.testResults) {
          assert.equal(res.status, "Success", `${res.name} failed`);
        }
      } else if (suite.id.name.includes("Eip712UnknownTest")) {
        assert.equal(suite.testResults[0].status, "Failure");
      } else {
        assert.fail("Unexpected test suite name: " + suite.id.name);
      }
    }
  });

  it("filters tests according to pattern", async function () {
    const artifacts = [
      loadContract("./data/artifacts/default/SetupConsistencyCheck.json"),
    ];
    // All artifacts are test suites.
    const testSuites = artifacts.map((artifact) => artifact.id);

    const [, results] = await runAllSolidityTests(
      context,
      L1_CHAIN_TYPE,
      artifacts,
      testSuites,
      {
        disableTransactionGasCap: true,
        projectRoot: __dirname,
        hardfork: l1HardforkToString(l1HardforkLatest()),
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
