import { assert } from "chai";
import { TestContext } from "./testContext";

describe("Unit tests", () => {
  let testContext: TestContext;

  before(async () => {
    testContext = await TestContext.setup();
  });

  it("SuccessAndFailure", async function () {
    const { totalTests, failedTests } = await testContext.runTestsWithStats(
      "SuccessAndFailureTest"
    );

    assert.equal(failedTests, 1);
    assert.equal(totalTests, 2);
  });

  it("ContractEnvironment", async function () {
    const { totalTests, failedTests } = await testContext.runTestsWithStats(
      "ContractEnvironmentTest",
      {
        sender: Buffer.from("976EA74026E726554dB657fA54763abd0C3a0aa9", "hex"),
        chainId: 12n,
        blockNumber: 23n,
        blockTimestamp: 45n,
      }
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  it("LastCallGasIsolated", async function () {
    const { totalTests, failedTests } = await testContext.runTestsWithStats(
      "LastCallGasIsolatedTest",
      {
        isolate: true,
      }
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 4);
  });

  it("GlobalFork", async function () {
    if (testContext.rpcUrl === undefined) {
      this.skip();
    }

    const { totalTests, failedTests } = await testContext.runTestsWithStats(
      "GlobalForkTest",
      {
        ethRpcUrl: testContext.rpcUrl,
        forkBlockNumber: 20_000_000n,
      }
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });

  it("ForkCheatcode", async function () {
    if (testContext.rpcUrl === undefined) {
      this.skip();
    }

    const { totalTests, failedTests } = await testContext.runTestsWithStats(
      "ForkCheatcodeTest",
      {
        rpcEndpoints: {
          alchemyMainnet: testContext.rpcUrl,
        },
      }
    );

    assert.equal(failedTests, 0);
    assert.equal(totalTests, 1);
  });
});
