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
        failedCorpusReplays: BigInt(0),
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

    assert(
      contractReport !== undefined,
      "SomeCounter contract should be in gas report"
    );

    assert.equal(contractReport.deployments.length, 1);
    const deployment = contractReport.deployments[0];
    assert(deployment.gas > 0n, "Deployment gas should be greater than 0");
    assert.equal(deployment.size, 510n);
    assert.equal(deployment.runtimeSize, 481n);
    assert.equal(deployment.status, GasReportExecutionStatus.Success);

    const incrementReports = contractReport.functions["increment()"];
    assert(
      incrementReports !== undefined,
      "increment() function should be in gas report"
    );
    assert.equal(incrementReports.length, 1);
    assert.equal(incrementReports[0].gas, 43_483n);
    assert.equal(incrementReports[0].status, GasReportExecutionStatus.Success);
    assert.equal(
      incrementReports[0].proxyChain.length,
      0,
      "increment() should have no proxy chain"
    );

    const numberReports = contractReport.functions["number()"];
    assert(
      numberReports !== undefined,
      "number() function should be in gas report"
    );
    assert.equal(numberReports.length, 2);
    for (const report of numberReports) {
      assert.equal(report.gas, 2_424n);
      assert.equal(report.status, GasReportExecutionStatus.Success);
      assert.equal(
        report.proxyChain.length,
        0,
        "number() should have no proxy chain"
      );
    }

    const setNumberReports = contractReport.functions["setNumber(uint256)"];
    assert(
      setNumberReports !== undefined,
      "setNumber(uint256) function should be in gas report"
    );
    assert.equal(setNumberReports.length, 2);
    for (const report of setNumberReports) {
      assert(
        report.gas > 0n,
        "setNumber(uint256) gas should be greater than 0"
      );
      assert.equal(report.status, GasReportExecutionStatus.Success);
      assert.equal(
        report.proxyChain.length,
        0,
        "setNumber(uint256) should have no proxy chain"
      );
    }
  });

  it("ProxyGasReportTest gas report", async function () {
    const result = await testContext.runTestsWithStats("ProxyGasReportTest", {
      generateGasReport: true,
    });

    const testResult = result.testResult;
    assert(testResult !== undefined);

    const gasReport = testResult.gasReport;
    assert(gasReport !== undefined);

    const proxyReport =
      gasReport.contracts["project/test-contracts/ProxyGasReport.t.sol:Proxy"];
    assert(proxyReport !== undefined, "Proxy contract should be in gas report");

    assert.equal(
      proxyReport.deployments.length,
      1,
      "Proxy should have one deployment"
    );
    assert.equal(
      Object.keys(proxyReport.functions).length,
      0,
      "Proxy should have no function calls as they are propagated to the Implementation contract"
    );

    const implReport =
      gasReport.contracts[
        "project/test-contracts/ProxyGasReport.t.sol:Implementation"
      ];

    assert(
      implReport !== undefined,
      "Implementation contract should be in gas report"
    );

    assert.equal(
      implReport.deployments.length,
      1,
      "Implementation should have one deployment"
    );
    assert.equal(
      Object.keys(implReport.functions).length,
      3,
      `Implementation should have 3 functions, got ${Object.keys(implReport.functions)}`
    );

    const incrementReports = implReport.functions["increment()"];
    assert.equal(incrementReports.length, 1);
    assert.equal(incrementReports[0].gas, 48_269n);
    assert.equal(incrementReports[0].status, GasReportExecutionStatus.Success);
    assert.deepEqual(incrementReports[0].proxyChain, [
      "project/test-contracts/ProxyGasReport.t.sol:Proxy",
      "project/test-contracts/ProxyGasReport.t.sol:Implementation",
    ]);

    const valueReports = implReport.functions["value()"];
    assert.equal(valueReports.length, 2);

    for (const report of valueReports) {
      assert.equal(report.gas, 7_191n);
      assert.equal(report.status, GasReportExecutionStatus.Success);
      assert.deepEqual(report.proxyChain, [
        "project/test-contracts/ProxyGasReport.t.sol:Proxy",
        "project/test-contracts/ProxyGasReport.t.sol:Implementation",
      ]);
    }

    const setValueReports = implReport.functions["setValue(uint256)"];
    assert.equal(setValueReports.length, 1);
    assert.equal(setValueReports[0].gas, 48_507n);
    assert.equal(setValueReports[0].status, GasReportExecutionStatus.Success);
    assert.deepEqual(setValueReports[0].proxyChain, [
      "project/test-contracts/ProxyGasReport.t.sol:Proxy",
      "project/test-contracts/ProxyGasReport.t.sol:Implementation",
    ]);
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

    // The OuterProxy should appear with fallback() calls
    const outerProxyReport =
      gasReport.contracts[
        "project/test-contracts/ProxyGasReport.t.sol:OuterProxy"
      ];

    assert(
      outerProxyReport !== undefined,
      "OuterProxy contract should be in gas report"
    );
    assert.equal(
      outerProxyReport.deployments.length,
      1,
      "OuterProxy should have one deployment"
    );

    const proxyReport =
      gasReport.contracts["project/test-contracts/ProxyGasReport.t.sol:Proxy"];

    assert.equal(
      proxyReport.deployments.length,
      1,
      "Proxy should have one deployment"
    );

    const implReport =
      gasReport.contracts[
        "project/test-contracts/ProxyGasReport.t.sol:Implementation"
      ];

    assert.equal(
      implReport.deployments.length,
      1,
      "Implementation should have one deployment"
    );
    assert.equal(
      Object.keys(implReport.functions).length,
      3,
      `Implementation should have 3 functions, got ${Object.keys(implReport.functions)}`
    );

    const incrementReports = implReport.functions["increment()"];
    assert.equal(incrementReports.length, 1);
    assert.equal(incrementReports[0].gas, 53_055n);
    assert.equal(incrementReports[0].status, GasReportExecutionStatus.Success);
    assert.deepEqual(incrementReports[0].proxyChain, [
      "project/test-contracts/ProxyGasReport.t.sol:OuterProxy",
      "project/test-contracts/ProxyGasReport.t.sol:Proxy",
      "project/test-contracts/ProxyGasReport.t.sol:Implementation",
    ]);

    const valueReports = implReport.functions["value()"];
    assert.equal(valueReports.length, 2);

    for (const report of valueReports) {
      assert.equal(report.gas, 11_980n);
      assert.equal(report.status, GasReportExecutionStatus.Success);
      assert.deepEqual(report.proxyChain, [
        "project/test-contracts/ProxyGasReport.t.sol:OuterProxy",
        "project/test-contracts/ProxyGasReport.t.sol:Proxy",
        "project/test-contracts/ProxyGasReport.t.sol:Implementation",
      ]);
    }

    const setValueReports = implReport.functions["setValue(uint256)"];
    assert.equal(setValueReports.length, 1);
    assert.equal(setValueReports[0].gas, 53_296n);
    assert.equal(setValueReports[0].status, GasReportExecutionStatus.Success);
    assert.deepEqual(setValueReports[0].proxyChain, [
      "project/test-contracts/ProxyGasReport.t.sol:OuterProxy",
      "project/test-contracts/ProxyGasReport.t.sol:Proxy",
      "project/test-contracts/ProxyGasReport.t.sol:Implementation",
    ]);
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
    assert.equal(contractReport.deployments[0].runtimeSize, 754n);
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

  it("setUp() success status in gas report", async function () {
    const result = await testContext.runTestsWithStats("SetUpSuccessTest", {
      generateGasReport: true,
    });

    const testResult = result.testResult;
    assert(testResult !== undefined);

    const gasReport = testResult.gasReport;
    assert(gasReport !== undefined);

    const testContractReport =
      gasReport.contracts[
        "project/test-contracts/SetUpStatus.t.sol:SetUpSuccessTest"
      ];

    assert(
      testContractReport !== undefined,
      "SetUpSuccessTest contract should be in gas report"
    );

    assert.equal(
      testContractReport.deployments.length,
      1,
      "SetUpSuccessTest should have one deployment"
    );

    const testDeployment = testContractReport.deployments[0];
    assert(testDeployment.gas > 0n, "Deployment gas should be greater than 0");
    assert.equal(
      testDeployment.status,
      GasReportExecutionStatus.Success,
      "Deployment status should be Success when setUp succeeds"
    );

    const contractReport =
      gasReport.contracts["project/test-contracts/SetUpStatus.t.sol:Deploy"];

    assert.equal(
      contractReport.deployments.length,
      2,
      "Deploy should have two deployments (one for each test)"
    );

    for (const deployment of contractReport.deployments) {
      assert(deployment.gas > 0n, "Deployment gas should be greater than 0");
      assert.equal(
        deployment.status,
        GasReportExecutionStatus.Success,
        "Deployment status should be Success when setUp succeeds"
      );
    }
  });

  it("setUp() revert status in gas report", async function () {
    const result = await testContext.runTestsWithStats("SetUpRevertTest", {
      generateGasReport: true,
    });

    const testResult = result.testResult;
    assert(testResult !== undefined);

    const gasReport = testResult.gasReport;
    assert(gasReport !== undefined);

    const testContractReport =
      gasReport.contracts[
        "project/test-contracts/SetUpStatus.t.sol:SetUpRevertTest"
      ];

    assert(
      testContractReport !== undefined,
      "SetUpRevertTest contract should be in gas report"
    );

    assert.equal(
      testContractReport.deployments.length,
      1,
      "SetUpRevertTest should have one deployment"
    );

    const testDeployment = testContractReport.deployments[0];
    assert(testDeployment.gas > 0n, "Deployment gas should be greater than 0");
    assert.equal(
      testDeployment.status,
      GasReportExecutionStatus.Revert,
      "Deployment status should be Revert when setUp reverts"
    );

    const contractReport =
      gasReport.contracts["project/test-contracts/SetUpStatus.t.sol:Deploy"];

    assert(
      contractReport === undefined,
      "Deploy should not be included in the gas report since a revert in setUp causes Solidity test execution to terminate"
    );
  });

  it("SameProxyWithDifferentImplementationsTest gas report", async function () {
    const result = await testContext.runTestsWithStats(
      "SameProxyWithDifferentImplementationsTest",
      {
        generateGasReport: true,
      }
    );

    const testResult = result.testResult;
    assert(testResult !== undefined);

    const gasReport = testResult.gasReport;
    assert(gasReport !== undefined);

    const proxy1Report =
      gasReport.contracts["project/test-contracts/ProxyGasReport.t.sol:Proxy"];

    assert(proxy1Report !== undefined, "Proxy1 should be in gas report");
    assert(
      proxy1Report.deployments.length === 2,
      "Proxy1 should have two deployments"
    );
    assert(
      Object.keys(proxy1Report.functions).length === 0,
      "Proxy1 should have no function calls as they are propagated to the implementations"
    );

    const impl1Report =
      gasReport.contracts["project/test-contracts/ProxyGasReport.t.sol:Impl1"];

    assert(impl1Report !== undefined, "Impl1 should be in gas report");
    assert(
      impl1Report.deployments.length === 1,
      "Impl1 should have one deployment"
    );

    const impl1Functions = impl1Report.functions;
    assert(
      Object.keys(impl1Functions).length === 1,
      `Impl1 should have one function, got ${Object.keys(impl1Functions)}`
    );

    const oneFuncReport = impl1Functions["one()"];
    assert(
      oneFuncReport !== undefined,
      "one() function should be in Impl1's gas report"
    );
    assert(
      oneFuncReport.length === 2,
      "one() function should have two calls in Impl1's gas report"
    );

    const oneFuncReportForProxyCall = oneFuncReport.find((report) =>
      report.proxyChain.some((proxy) => proxy.includes(":Proxy"))
    );

    assert(
      oneFuncReportForProxyCall !== undefined,
      "one() function should have a call with Proxy in the proxy chain"
    );
    assert(
      oneFuncReportForProxyCall.proxyChain.length === 2,
      "Proxy call should have a 2-entry proxy chain"
    );

    const oneFuncReportForDirectCall = oneFuncReport.find(
      (report) => report.proxyChain.length === 0
    );
    assert(
      oneFuncReportForDirectCall !== undefined,
      "one() function should have a direct call without proxy chain"
    );

    assert(
      oneFuncReportForProxyCall.gas > oneFuncReportForDirectCall.gas,
      "Proxy call should have higher gas than direct call"
    );

    const impl2Report =
      gasReport.contracts["project/test-contracts/ProxyGasReport.t.sol:Impl2"];

    assert(impl2Report !== undefined, "Impl2 should be in gas report");
    assert(
      impl2Report.deployments.length === 1,
      "Impl2 should have one deployment"
    );

    const impl2Functions = impl2Report.functions;
    assert(
      Object.keys(impl2Functions).length === 1,
      `Impl2 should have one function, got ${Object.keys(impl2Functions)}`
    );

    const twoFuncReport = impl2Functions["two()"];
    assert(
      twoFuncReport !== undefined,
      "two() function should be in Impl2's gas report"
    );
    assert(
      twoFuncReport.length === 1,
      "two() function should have one call in Impl2's gas report"
    );
  });

  it("SameImplementationWithDifferentProxyChainsTest gas report", async function () {
    const result = await testContext.runTestsWithStats(
      "SameImplementationWithDifferentProxyChainsTest",
      {
        generateGasReport: true,
      }
    );

    const testResult = result.testResult;
    assert(testResult !== undefined);

    const gasReport = testResult.gasReport;
    assert(gasReport !== undefined);

    const proxy1Report =
      gasReport.contracts["project/test-contracts/ProxyGasReport.t.sol:Proxy"];

    assert(proxy1Report !== undefined, "Proxy should be in gas report");
    assert(
      proxy1Report.deployments.length === 2,
      "Proxy should have two deployments"
    );
    assert(
      Object.keys(proxy1Report.functions).length === 0,
      "Proxy should have no function calls as they are propagated to the implementations"
    );

    const impl1Report =
      gasReport.contracts["project/test-contracts/ProxyGasReport.t.sol:Impl1"];

    assert(impl1Report !== undefined, "Impl1 should be in gas report");
    assert(
      impl1Report.deployments.length === 1,
      "Impl1 should have one deployment"
    );

    const impl1Functions = impl1Report.functions;
    assert(
      Object.keys(impl1Functions).length === 1,
      `Impl1 should have one function, got ${Object.keys(impl1Functions)}`
    );

    const oneFuncReport = impl1Functions["one()"];
    assert(
      oneFuncReport !== undefined,
      "one() function should be in Impl1's gas report"
    );
    assert(
      oneFuncReport.length === 2,
      "one() function should have two calls in Impl1's gas report"
    );

    const oneFuncReportWithSingleProxy = oneFuncReport.find(
      (report) => report.proxyChain.length === 2
    );

    assert(
      oneFuncReportWithSingleProxy !== undefined,
      "one() function should have a call with only Proxy1 in the proxy chain"
    );
    assert(
      oneFuncReportWithSingleProxy.proxyChain.length === 2,
      "Proxy call should have a 2-entry proxy chain"
    );

    const oneFuncReportWithTwoProxies = oneFuncReport.find(
      (report) => report.proxyChain.length === 3
    );

    assert(
      oneFuncReportWithTwoProxies !== undefined,
      "one() function should have a call with both OuterProxy and Proxy in the proxy chain"
    );
    assert(
      oneFuncReportWithTwoProxies.proxyChain.length === 3,
      "Proxy call should have a 3-entry proxy chain"
    );
    assert(
      oneFuncReportWithTwoProxies.proxyChain[0].includes("OuterProxy") &&
        oneFuncReportWithTwoProxies.proxyChain[1].includes("Proxy"),
      "Expected proxy chain to include both OuterProxy and Proxy"
    );
    assert(
      oneFuncReportWithTwoProxies.gas > oneFuncReportWithSingleProxy.gas,
      "Call with two proxies should have higher gas than call with one proxy"
    );
  });

  it("runtimeSize is constant across deployments with different constructor args", async function () {
    const contractKey = "project/contracts/Greeter.sol:Greeter";

    // Short greeting: ABI args = offset(32) + length(32) + data(32) = 96 bytes
    const shortResult = await testContext.runTestsWithStats(
      "RuntimeSizeShortTest",
      { generateGasReport: true }
    );
    const shortReport =
      shortResult.testResult!.gasReport!.contracts[contractKey];
    assert(shortReport !== undefined, "Greeter should be in short test report");

    // Long greeting (33 chars): ABI args = offset(32) + length(32) + data(64) = 128 bytes
    const longResult = await testContext.runTestsWithStats(
      "RuntimeSizeLongTest",
      { generateGasReport: true }
    );
    const longReport = longResult.testResult!.gasReport!.contracts[contractKey];
    assert(longReport !== undefined, "Greeter should be in long test report");

    const shortDeploy = shortReport.deployments[0];
    const longDeploy = longReport.deployments[0];

    // Size includes constructor args, so it differs between deployments (one extra ABI slot for the longer string)
    assert.equal(
      longDeploy.size - shortDeploy.size,
      32n,
      `Expected size difference of 32 bytes for the additional constructor argument data, got ${longDeploy.size - shortDeploy.size} bytes`
    );

    assert.equal(
      shortDeploy.runtimeSize,
      531n,
      `Expected runtimeSize of 531 bytes, got ${shortDeploy.runtimeSize} bytes`
    );

    // runtimeSize must be identical
    assert.equal(
      shortDeploy.runtimeSize,
      longDeploy.runtimeSize,
      "runtimeSize should be the same regardless of constructor args"
    );
  });
});
