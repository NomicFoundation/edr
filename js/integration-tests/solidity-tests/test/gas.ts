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
        metrics: {},
        failedCorpusReplays: BigInt(0)
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

    const testResult = result.testResult;
    assert(testResult !== undefined);

    const gasReport = testResult.gasReport;
    assert(gasReport !== undefined);

    const contractReport =
      gasReport.contracts["project/test-contracts/Counter.t.sol:SomeCounter"];

    assert.equal(contractReport.deployments.length, 1);
    assert(contractReport.deployments[0].gas > 0n);
    assert.equal(contractReport.deployments[0].size, 510n);
    assert.equal(
      contractReport!.deployments[0].status,
      GasReportExecutionStatus.Success
    );

    assert(contractReport.functions["increment()"] !== undefined);
    assert(contractReport.functions["number()"] !== undefined);
    assert(contractReport.functions["setNumber(uint256)"] !== undefined);

    const incrementReports = contractReport.functions["increment()"];
    assert.equal(incrementReports.length, 1);
    assert.equal(incrementReports[0].gas, 43_483n);
    assert.equal(incrementReports[0].status, GasReportExecutionStatus.Success);
  });

  it("ProxyGasReportTest gas report", async function () {
    const result = await testContext.runTestsWithStats("ProxyGasReportTest", {
      generateGasReport: true,
    });

    const testResult = result.testResult;
    assert(testResult !== undefined);

    const gasReport = testResult.gasReport;
    assert(gasReport !== undefined);

    // Debug: print all contracts and their functions in the gas report
    for (const [name, report] of Object.entries(gasReport.contracts)) {
      console.log(`Contract: ${name}`);
      for (const [funcName, funcReports] of Object.entries(report.functions)) {
        console.log(`  Function: ${funcName}, calls: ${funcReports.length}`);
        for (const r of funcReports) {
          console.log(`    gas: ${r.gas}, proxyChain: ${JSON.stringify(r.proxyChain)}`);
        }
      }
    }

    // The Proxy contract should appear in the gas report
    const proxyReport =
      gasReport.contracts[
        "project/test-contracts/ProxyGasReport.t.sol:Proxy"
      ];
    assert(proxyReport !== undefined, "Proxy contract should be in gas report");

    // The proxy's functions are decoded as fallback() since the Proxy ABI
    // only has a fallback function (the actual function selectors belong to
    // the Implementation contract).
    const fallbackReports = proxyReport.functions["fallback()"];
    assert(
      fallbackReports !== undefined,
      "fallback should appear in Proxy's gas report"
    );
    // 4 calls: setValue, value (in test_proxySetValue), increment, value (in test_proxyIncrement)
    assert.equal(fallbackReports.length, 4);

    // All fallback calls should have a proxy chain with 2 entries
    // (Proxy -> Implementation) indicating the delegation pattern was detected.
    for (const report of fallbackReports) {
      assert.equal(
        report.proxyChain.length,
        2,
        `Expected proxy chain with 2 entries, got ${report.proxyChain.length}: ${JSON.stringify(report.proxyChain)}`
      );
      assert(
        report.proxyChain[0].includes("Proxy"),
        `Expected first chain entry to contain 'Proxy', got '${report.proxyChain[0]}'`
      );
      assert(
        report.proxyChain[1].includes("Implementation"),
        `Expected second chain entry to contain 'Implementation', got '${report.proxyChain[1]}'`
      );
    }

    // Verify that the Implementation contract's direct calls (via delegatecall)
    // have empty proxy chains
    const implReport =
      gasReport.contracts[
        "project/test-contracts/ProxyGasReport.t.sol:Implementation"
      ];
    assert(
      implReport !== undefined,
      "Implementation contract should be in gas report"
    );
    for (const [, funcReports] of Object.entries(implReport.functions)) {
      for (const report of funcReports) {
        assert.equal(
          report.proxyChain.length,
          0,
          "Direct delegatecall targets should have empty proxy chain"
        );
      }
    }
  });

  it("ChainedProxyGasReportTest gas report", async function () {
    const result = await testContext.runTestsWithStats(
      "ChainedProxyGasReportTest",
      {
        generateGasReport: true,
      }
    );

    const testResult = result.testResult;
    assert(testResult !== undefined);

    const gasReport = testResult.gasReport;
    assert(gasReport !== undefined);

    // Debug: print all contracts and their functions in the gas report
    for (const [name, report] of Object.entries(gasReport.contracts)) {
      console.log(`Contract: ${name}`);
      for (const [funcName, funcReports] of Object.entries(report.functions)) {
        console.log(`  Function: ${funcName}, calls: ${funcReports.length}`);
        for (const r of funcReports) {
          console.log(
            `    gas: ${r.gas}, proxyChain: ${JSON.stringify(r.proxyChain)}`
          );
        }
      }
    }

    // The OuterProxy should appear with fallback() calls
    const outerProxyReport =
      gasReport.contracts[
        "project/test-contracts/ProxyGasReport.t.sol:OuterProxy"
      ];
    assert(
      outerProxyReport !== undefined,
      "OuterProxy contract should be in gas report"
    );

    const outerFallbackReports = outerProxyReport.functions["fallback()"];
    assert(
      outerFallbackReports !== undefined,
      "fallback should appear in OuterProxy's gas report"
    );

    // All OuterProxy fallback calls should have a 3-entry proxy chain:
    // OuterProxy -> Proxy -> Implementation
    for (const report of outerFallbackReports) {
      assert.equal(
        report.proxyChain.length,
        3,
        `Expected proxy chain with 3 entries, got ${report.proxyChain.length}: ${JSON.stringify(report.proxyChain)}`
      );
      assert(
        report.proxyChain[0].includes("OuterProxy"),
        `Expected first entry to be OuterProxy, got '${report.proxyChain[0]}'`
      );
      assert(
        report.proxyChain[1].includes("Proxy"),
        `Expected second entry to be Proxy, got '${report.proxyChain[1]}'`
      );
      assert(
        report.proxyChain[2].includes("Implementation"),
        `Expected third entry to be Implementation, got '${report.proxyChain[2]}'`
      );
    }
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
    assert(contractReport.deployments[0].gas > 0n);
    assert.equal(contractReport.deployments[0].size, 783n);
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
    assert.equal(addToAReports[0].gas, 22_978n);
    assert.equal(addToAReports[0].status, GasReportExecutionStatus.Success);
  });
});
