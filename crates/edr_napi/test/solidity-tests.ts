import { assert } from "chai";

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

  it("exposes the stack-trace kind tag for a failing test", async function () {
    const artifacts = [
      loadContract("./data/artifacts/default/PaymentFailureTest.json"),
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

    const failure = results[0].testResults[0];
    assert.equal(failure.status, "Failure");

    const trace = failure.stackTrace();
    if (trace === null) {
      // collectStackTraces defaults to OnFailure, so a failing test must
      // produce a stack trace; null would make the kind assertion vacuous.
      assert.fail("expected a stack-trace result for the failing test");
    } else {
      assert.oneOf(trace.kind, [
        "StackTrace",
        "UnexpectedError",
        "HeuristicFailed",
        "UnsafeToReplay",
      ]);
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

  it("rejects invalid eip712CanonicalTypes as InvalidArg", async function () {
    // Boundary check only: an invalid eip712CanonicalTypes entry must
    // reject with an InvalidArg error. The exhaustive semantics
    // (collecting every bad entry, duplicate detection, etc.) are covered
    // by `parse_eip712_canonical_types` unit tests in the cheatcodes crate.
    const artifacts = [
      loadContract("./data/artifacts/default/SetupConsistencyCheck.json"),
    ];
    const testSuites = artifacts.map((artifact) => artifact.id);
    const config = {
      projectRoot: __dirname,
      hardfork: l1HardforkToString(l1HardforkLatest()),
      eip712CanonicalTypes: ["gibberish"],
    };

    let error: any;
    try {
      await runAllSolidityTests(
        context,
        L1_CHAIN_TYPE,
        artifacts,
        testSuites,
        config
      );
    } catch (e) {
      error = e;
    }

    assert.isDefined(error);
    assert.equal(error.code, "InvalidArg");
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

  it("excludes tests according to pattern", async function () {
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
        excludeTestPattern: "Multiply",
      }
    );

    assert.equal(results.length, artifacts.length);

    for (const res of results) {
      if (res.id.name.includes("SetupConsistencyCheck")) {
        assert.equal(res.testResults.length, 1);
        assert.equal(res.testResults[0].name, "testAdd()");
      } else {
        assert.fail("Unexpected test suite name: " + res.id.name);
      }
    }
  });

  it("combines testPattern and excludeTestPattern", async function () {
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
        // `testPattern` selects both `testAdd` and `testMultiply`, while
        // `excludeTestPattern` removes `testMultiply`, leaving only `testAdd`.
        testPattern: "test",
        excludeTestPattern: "Multiply",
      }
    );

    assert.equal(results.length, artifacts.length);

    for (const res of results) {
      if (res.id.name.includes("SetupConsistencyCheck")) {
        assert.equal(res.testResults.length, 1);
        assert.equal(res.testResults[0].name, "testAdd()");
      } else {
        assert.fail("Unexpected test suite name: " + res.id.name);
      }
    }
  });

  // Pins `#[napi(async_runtime)]` on the sync entry points so a future
  // entry point added without it panics on first CI run. Without the
  // attribute, `tokio::Handle::current()` panics from microtask
  // callbacks ("there is no reactor running, must be called from the
  // context of a Tokio 1.x runtime"). Existing tests cover
  // `createProvider`, `runSolidityTests`, `createMockProvider`, and
  // `createProviderWithMockTimer` implicitly via async/await; this one
  // makes the requirement explicit by entering through `queueMicrotask`.
  it("entry points are callable from microtask context (async_runtime regression)", async function () {
    await new Promise<void>((resolve) => queueMicrotask(resolve));

    const artifacts = [
      loadContract("./data/artifacts/default/SetupConsistencyCheck.json"),
    ];
    const testSuites = artifacts.map((artifact) => artifact.id);
    const [, results] = await runAllSolidityTests(
      context,
      L1_CHAIN_TYPE,
      artifacts,
      testSuites,
      {
        projectRoot: __dirname,
        hardfork: l1HardforkToString(l1HardforkLatest()),
      }
    );

    assert.equal(results.length, artifacts.length);
  });
});
