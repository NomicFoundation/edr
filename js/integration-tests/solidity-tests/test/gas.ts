import assert from "node:assert/strict";
import { before, describe, it } from "node:test";
import {
  StandardTestKind,
  FuzzTestKind,
  InvariantTestKind,
  GasReportExecutionStatus,
} from "@nomicfoundation/edr";

import {
  extractGasUsage,
  GasUsageFilter,
  SortOrder,
} from "@nomicfoundation/edr/solidity-tests";
import { TestContext } from "./testContext.js";

describe("Gas tests", () => {
  let testResults: {
    name: string;
    kind: StandardTestKind | FuzzTestKind | InvariantTestKind;
  }[] = [
    {
      name: "Standard Test",
      kind: {
        consumedGas: BigInt(1000),
      },
    },
    {
      name: "Fuzz Test",
      kind: {
        runs: BigInt(10),
        meanGas: BigInt(2000),
        medianGas: BigInt(1500),
      },
    },
    {
      name: "Invariant Test",
      kind: {
        runs: BigInt(5),
        calls: BigInt(100),
        reverts: BigInt(0),
      },
    },
  ];

  it("basic extractGasUsage", async function () {
    const simpleGasUsage = extractGasUsage(testResults);
    assert.equal(simpleGasUsage.length, 3);
    assert.deepEqual(simpleGasUsage[0], {
      name: "Standard Test",
      gas: BigInt(1000),
    });
    assert.deepEqual(simpleGasUsage[1], {
      name: "Fuzz Test",
      gas: BigInt(1500), // medianGas for Fuzz Test
    });
    assert.deepEqual(simpleGasUsage[2], {
      name: "Invariant Test",
      gas: BigInt(0), // Default for Invariant Test
    });
  });

  it("filtered extractGasUsage", async function () {
    const filter: GasUsageFilter = {
      minThreshold: BigInt(1),
      maxThreshold: BigInt(1000),
    };
    const filteredGasUsage = extractGasUsage(testResults, filter);

    assert.equal(filteredGasUsage.length, 1);
    assert.deepEqual(filteredGasUsage[0], {
      name: "Standard Test",
      gas: BigInt(1000),
    });
  });

  it("sorted extractGasUsage", async function () {
    const ascendingGasUsage = extractGasUsage(
      testResults,
      undefined,
      SortOrder.Ascending
    );

    assert.equal(ascendingGasUsage.length, 3);
    assert.deepEqual(ascendingGasUsage[0], {
      name: "Invariant Test",
      gas: BigInt(0), // Default for Invariant Test
    });
    assert.deepEqual(ascendingGasUsage[1], {
      name: "Standard Test",
      gas: BigInt(1000),
    });
    assert.deepEqual(ascendingGasUsage[2], {
      name: "Fuzz Test",
      gas: BigInt(1500), // medianGas for Fuzz Test
    });
  });

  it("filtered and sorted extractGasUsage", async function () {
    const filter: GasUsageFilter = {
      minThreshold: BigInt(1), // exclude Invariant Test
    };
    const gasUsage = extractGasUsage(testResults, filter, SortOrder.Descending);

    assert.equal(gasUsage.length, 2);
    assert.deepEqual(gasUsage[0], {
      name: "Fuzz Test",
      gas: BigInt(1500), // medianGas for Fuzz Test
    });
    assert.deepEqual(gasUsage[1], {
      name: "Standard Test",
      gas: BigInt(1000),
    });
  });
});

describe("Gas report tests", () => {
  let testContext: TestContext;

  before(async function () {
    testContext = await TestContext.setup();
  });

  it("CounterTest gas report", async function () {
    const result = await testContext.runTestsWithStats("CounterTest", {
      generateGasReport: true,
    });
    assert.equal(result.failedTests, 0);
    assert.equal(result.totalTests, 2);

    const testResult = result.testResult;
    assert(testResult !== undefined);

    const gasReport = testResult.gasReport;
    assert(gasReport !== undefined);

    const contractReport =
      gasReport.contracts["project/test-contracts/Counter.t.sol:SomeCounter"];

    assert.equal(contractReport.deployments.length, 1);
    assert.equal(contractReport.deployments[0].gas, BigInt(156817));
    assert.equal(contractReport.deployments[0].size, BigInt(510));
    assert.equal(
      contractReport!.deployments[0].status,
      GasReportExecutionStatus.Success
    );

    assert(contractReport.functions["increment()"] !== undefined);
    assert(contractReport.functions["number()"] !== undefined);
    assert(contractReport.functions["setNumber(uint256)"] !== undefined);

    const incrementReports = contractReport.functions["increment()"];
    assert.equal(incrementReports.length, 1);
    assert.equal(incrementReports[0].gas, BigInt(43483));
    assert.equal(incrementReports[0].status, GasReportExecutionStatus.Success);
  });

  it("ImpureInvariantTest gas report", async function () {
    const invariantConfig = {
      runs: 256,
      depth: 15,
      // This is false by default, we just specify it here to make it obvious to the reader.
      failOnRevert: false,
    };

    const result = await testContext.runTestsWithStats("ImpureInvariantTest", {
      invariant: invariantConfig,
      generateGasReport: true,
    });
    assert.equal(result.failedTests, 1);
    assert.equal(result.totalTests, 1);

    const testResult = result.testResult;
    assert(testResult !== undefined);

    const gasReport = testResult.gasReport;
    assert(gasReport !== undefined);

    // This is the contract that contains non setup/test functions.
    const contractReport =
      gasReport.contracts[
        "project/test-contracts/Invariant.t.sol:StochasticWrongContract"
      ];

    assert.equal(contractReport.deployments.length, 1);
    assert.equal(contractReport.deployments[0].gas, BigInt(215576));
    assert.equal(contractReport.deployments[0].size, BigInt(783));
    assert.equal(
      contractReport!.deployments[0].status,
      GasReportExecutionStatus.Success
    );

    assert(contractReport.functions["a()"] !== undefined);
    assert(contractReport.functions["b()"] !== undefined);
    assert(contractReport.functions["addToA(uint256)"] !== undefined);
    assert(contractReport.functions["both()"] !== undefined);

    const addToAReports = contractReport.functions["addToA(uint256)"];
    assert.equal(addToAReports.length, 1);
    assert.equal(addToAReports[0].gas, BigInt(22978));
    assert.equal(addToAReports[0].status, GasReportExecutionStatus.Success);
  });
});
