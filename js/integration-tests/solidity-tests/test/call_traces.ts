import assert from "node:assert/strict";
import test, { before, describe, it } from "node:test";
import { TestContext } from "./testContext.js";
import {
  IncludeTraces,
  CallKind,
  LogKind,
  CallTrace,
} from "@nomicfoundation/edr";

describe("Call traces - IncludeTraces.All", () => {
  let testCallTraces: Map<string, CallTrace[]>;

  before(async () => {
    const testContext = await TestContext.setup();
    const runResult = await testContext.runTestsWithStats("CallTraces", {
      includeTraces: IncludeTraces.All,
    });
    testCallTraces = runResult.callTraces;
  });

  it("no children", async function () {
    const trace = testCallTraces.get("testNoChildren()");
    assert.equal(trace?.length, 1);
    assert.deepEqual(trace[0], {
      kind: CallKind.Call,
      success: true,
      isCheatcode: false,
      gasUsed: trace[0].gasUsed, // avoid coupling test to specific gas costs
      value: 0n,
      address: '0x7FA9385bE102ac3EAc297483Dd6233D62b3e1496',
      contract: "CallTraces",
      inputs: {
        name: "testNoChildren",
        arguments: [],
      },
      outputs: new Uint8Array(0),
      children: [],
    });
  });

  it("single child call", async function () {
    const trace = testCallTraces.get("testSingleChildCall()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);
    const child = trace[0].children[0];
    assert.equal(child.kind, CallKind.Call);
    assert.deepEqual(child.inputs, { name: "childCall", arguments: ["55"] });
    assert.deepEqual(child.outputs, "365");
  });

  it("single event", async function () {
    const trace = testCallTraces.get("testSingleEvent()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);
    const event = trace[0].children[0];
    assert.equal(event.kind, LogKind.Log);
    assert.deepEqual(event.parameters, {
      name: "SomeEvent",
      arguments: ["x: 123", 's: "hello"'],
    });
  });

  it("multiple children", async function () {
    const trace = testCallTraces.get("testManyChildren()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 5);

    const child0 = trace[0].children[0];
    assert.equal(child0.kind, LogKind.Log);
    assert.deepEqual(child0.parameters, {
      name: "OneEvent",
      arguments: ["x: 1"],
    });

    const child1 = trace[0].children[1];
    assert.equal(child1.kind, CallKind.Call);
    assert.deepEqual(child1.inputs, { name: "childCall", arguments: ["2"] });

    const child2 = trace[0].children[2];
    assert.equal(child2.kind, LogKind.Log);
    assert.deepEqual(child2.parameters, {
      name: "OneEvent",
      arguments: ["x: 3"],
    });

    const child3 = trace[0].children[3];
    assert.equal(child3.kind, CallKind.Call);
    assert.deepEqual(child3.inputs, { name: "childCall", arguments: ["4"] });

    const child4 = trace[0].children[4];
    assert.equal(child4.kind, LogKind.Log);
    assert.deepEqual(child4.parameters, {
      name: "OneEvent",
      arguments: ["x: 5"],
    });
  });

  it("nested calls", async function () {
    const trace = testCallTraces.get("testNestedCalls()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);

    const child = trace[0].children[0];
    assert.equal(child.kind, CallKind.Call);
    assert.deepEqual(child.inputs, { name: "nestedCall", arguments: [] });
    assert.equal(child.children.length, 1);

    const grandChild = child.children[0];
    assert.equal(grandChild.kind, CallKind.Call);
    assert.deepEqual(grandChild.inputs, {
      name: "childCall",
      arguments: ["0"],
    });
  });

  it("call with value", async function () {
    const trace = testCallTraces.get("testCallWithValue()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 2);

    // First child is vm.deal cheatcode
    const dealCall = trace[0].children[0];
    assert.equal(dealCall.kind, CallKind.Call);
    assert.equal(dealCall.isCheatcode, true);

    // Second child is the transfer call
    const transferCall = trace[0].children[1];
    assert.equal(transferCall.kind, CallKind.Call);
    assert.equal(transferCall.value, 1000000000000000000n); // 1 ether in wei
  });

  it("cheatcode call", async function () {
    const trace = testCallTraces.get("testCheatcodeCall()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);

    const child = trace[0].children[0];
    assert.equal(child.kind, CallKind.StaticCall);
    assert.equal(child.isCheatcode, true);
    assert.deepEqual(child.inputs, { name: "addr", arguments: ["<pk>"] });
  });

  it("labeled address", async function () {
    const trace = testCallTraces.get("testLabelAddress()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 2);

    const labelCall = trace[0].children[0];
    assert.equal(labelCall.kind, CallKind.Call);
    assert.equal(labelCall.isCheatcode, true);

    const targetCall = trace[0].children[1];
    assert.equal(targetCall.kind, CallKind.Call);
    assert.equal(targetCall.contract, "a labelled someone");
  });

  it("raw bytes call", async function () {
    const trace = testCallTraces.get("testRawBytesCall()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);

    const child = trace[0].children[0];
    assert.equal(child.kind, CallKind.Call);
    assert.deepEqual(child.inputs, {
      arguments: ["0xdeadbeef"],
      name: "fallback"
    });
  });

  it("undecoded outputs", async function () {
    const trace = testCallTraces.get("testUndecodedOutputs()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);

    const child = trace[0].children[0];
    assert.equal(child.kind, CallKind.Call);
    assert.deepEqual(child.outputs, new Uint8Array([0x12, 0x34, 0x00, 0x42]));
  });

  it("anonymous event", async function () {
    const trace = testCallTraces.get("testAnonymousEvent()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);

    const event = trace[0].children[0];
    assert.equal(event.kind, LogKind.Log);
    assert(Array.isArray(event.parameters));
    assert.equal(event.parameters.length, 3); // 2 indexed topics + data

    assert.deepEqual(
      event.parameters[0],
      new Uint8Array(
        Buffer.from(
          "0000000000000000000000000000000000000000000000000000000000000001",
          "hex"
        )
      )
    );
    assert.deepEqual(
      event.parameters[1],
      new Uint8Array(
        Buffer.from(
          "0000000000000000000000000000000000000000000000000000000000000002",
          "hex"
        )
      )
    );

    const data = "test data";
    assert.deepEqual(
      event.parameters[2],
      new Uint8Array(
        Buffer.concat([
          Buffer.from(
            "0000000000000000000000000000000000000000000000000000000000000020",
            "hex"
          ), // start offset
          Buffer.from(data.length.toString(16).padStart(64, "0"), "hex"),
          Buffer.from(data.padEnd(32, "\0"), "utf8"),
        ])
      )
    );
  });

  it("create contract", async function () {
    const trace = testCallTraces.get("testCreateContract()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);

    const child = trace[0].children[0];
    assert.equal(child.kind, CallKind.Create);
    assert.equal(child.success, true);
    assert.equal(child.contract, "CreateMe");
    assert.equal(child.address, "0x5615dEB798BB3E4dFa0139dFa1b3D433Cc23b72f");
    assert(typeof child.outputs === "string");
    assert.match(child.outputs, /^\d+ bytes of code$/);
  });

  it("static and delegate calls", async function () {
    const trace = testCallTraces.get("testStaticAndDelegateCall()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 2);

    const staticCall = trace[0].children[0];
    assert.equal(staticCall.kind, CallKind.StaticCall);
    assert.deepEqual(staticCall.inputs, { name: "simpleCall", arguments: [] });

    const delegateCall = trace[0].children[1];
    assert.equal(delegateCall.kind, CallKind.DelegateCall);
    assert.deepEqual(delegateCall.inputs, {
      name: "simpleCall",
      arguments: [],
    });
  });

  it("reverted calls", async function () {
    const trace = testCallTraces.get("testRevertedCall()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 4);

    const emptyRevert = trace[0].children[0];
    assert.equal(emptyRevert.kind, CallKind.Call);
    assert.equal(emptyRevert.success, false);
    assert.deepEqual(emptyRevert.inputs, {
      name: "revertWithEmpty",
      arguments: [],
    });
    assert.equal(emptyRevert.outputs, "EvmError: Revert");

    const stringRevert = trace[0].children[1];
    assert.equal(stringRevert.kind, CallKind.Call);
    assert.equal(stringRevert.success, false);
    assert.deepEqual(stringRevert.inputs, {
      name: "revertWithString",
      arguments: [],
    });
    assert.equal(stringRevert.outputs, "Something went wrong");

    const customErrorRevert = trace[0].children[2];
    assert.equal(customErrorRevert.kind, CallKind.Call);
    assert.equal(customErrorRevert.success, false);
    assert.deepEqual(customErrorRevert.inputs, {
      name: "revertWithCustomError",
      arguments: [],
    });
    assert.equal(
      customErrorRevert.outputs,
      'CustomRevertError(42, "Custom error occurred")'
    );

    const bytesRevert = trace[0].children[3];
    assert.equal(bytesRevert.kind, CallKind.Call);
    assert.equal(bytesRevert.success, false);
    assert.deepEqual(bytesRevert.inputs, {
      name: "revertWithBytes",
      arguments: [],
    });
    assert.deepEqual(bytesRevert.outputs, "custom error 0xdeadbeef: cafe");
  });

  it("reverted contract creation", async function () {
    const trace = testCallTraces.get("testRevertedCreate()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);

    const revertedCreate = trace[0].children[0];
    assert.equal(revertedCreate.kind, CallKind.Create);
    assert.equal(revertedCreate.success, false);
  });

  it("unlabeled address", async function () {
    const trace = testCallTraces.get("testUnlabeledAddress()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);

    const unlabeledCall = trace[0].children[0];
    assert.equal(unlabeledCall.kind, CallKind.Call);
    assert.equal(unlabeledCall.success, true);
    assert.equal(unlabeledCall.contract, undefined);
    assert.equal(
      unlabeledCall.address,
      "0xaBcDef1234567890123456789012345678901234"
    );
  });

  it("empty call data", async function () {
    const trace = testCallTraces.get("testEmptyCallData()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 4);

    // receive in ABI
    const emptyCall1 = trace[0].children[0];
    assert.equal(emptyCall1.kind, CallKind.Call);
    assert.equal(emptyCall1.success, true);
    assert.equal(emptyCall1.contract, "CallTraces");
    assert.deepEqual(emptyCall1.inputs, { name: "receive", arguments: [] });

    // fallback in ABI
    const emptyCall2 = trace[0].children[2];
    assert.equal(emptyCall2.kind, CallKind.Call);
    assert.equal(emptyCall2.success, true);
    assert.deepEqual(emptyCall2.inputs, { name: "fallback", arguments: [] });

    // no ABI
    const emptyCall3 = trace[0].children[3];
    assert.equal(emptyCall3.kind, CallKind.Call);
    assert.equal(emptyCall3.success, true);
    assert.equal(emptyCall3.contract, undefined);
    assert.deepEqual(emptyCall3.inputs, new Uint8Array(0));
  });

  it("fuzzing test should have single trace", async function () {
    const trace = testCallTraces.get("testWithFuzzing(uint256)");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].kind, CallKind.Call);
    assert.equal(trace[0].success, true);
    assert.equal(trace[0].contract, "CallTraces");
    assert.equal(trace[0].children.length, 1);

    const event = trace[0].children[0];
    assert.equal(event.kind, LogKind.Log);
    assert("name" in event.parameters);
    assert.deepEqual(event.parameters.name, "OneEvent");
    const argumentMatch = event.parameters.arguments[0].match(/^x: (.*)$/);
    assert.ok(argumentMatch);
    const x = argumentMatch[1];

    assert("name" in trace[0].inputs);
    assert.deepEqual(trace[0].inputs, {
      name: "testWithFuzzing",
      arguments: [x],
    });
  });
});

describe("Call traces - IncludeTraces.Failing", () => {
  let testCallTraces: Map<string, CallTrace[]>;

  before(async () => {
    const testContext = await TestContext.setup();
    const runResult = await testContext.runTestsWithStats(
      "CallTracesFailingOnly",
      { includeTraces: IncludeTraces.Failing }
    );
    testCallTraces = runResult.callTraces;
  });

  it("should not capture traces for successful tests", async function () {
    const trace = testCallTraces.get("testSuccessfulTest()");
    assert.deepEqual(trace, []);
  });

  it("should capture traces for failing tests", async function () {
    const trace = testCallTraces.get("testIntentionallyFailingTest()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].success, false);
  });
});

describe("Call traces - CallTracesSetup", () => {
  let testCallTraces: Map<string, CallTrace[]>;

  before(async () => {
    const testContext = await TestContext.setup();
    const runResult = await testContext.runTestsWithStats("CallTracesSetup", {
      includeTraces: IncludeTraces.All,
    });
    testCallTraces = runResult.callTraces;
  });

  it("should include setUp function in traces", async function () {
    const trace = testCallTraces.get("testAfterSetup()");
    assert.equal(trace?.length, 2);

    const setupTrace = trace[0];
    assert.equal(setupTrace.kind, CallKind.Call);
    assert.equal(setupTrace.success, true);
    assert.equal(setupTrace.contract, "CallTracesSetup");
    assert.deepEqual(setupTrace.inputs, { name: "setUp", arguments: [] });

    const testTrace = trace[1];
    assert.equal(testTrace.kind, CallKind.Call);
    assert.equal(testTrace.success, true);
    assert.equal(testTrace.contract, "CallTracesSetup");
    assert.deepEqual(testTrace.inputs, {
      name: "testAfterSetup",
      arguments: [],
    });
  });
});

describe("Pause and Resume Tracing", () => {
  let testCallTraces: Map<string, CallTrace[]>;

  before(async () => {
    const testContext = await TestContext.setup();
    const runResult = await testContext.runTestsWithStats("PauseTracingTest", {
      includeTraces: IncludeTraces.All,
      isolate: true,
    });
    testCallTraces = runResult.callTraces;
  });

  it("should have fewer traces", async function () {
    const setUpTrace = testCallTraces.get("test()")![0];
    // Not pausing tracing would result in 3 traces here
    assert.equal(setUpTrace.children.length, 2);

    const testTrace = testCallTraces.get("test()")![1];
    // Not pausing tracing would result in 3 traces here
    assert.equal(testTrace.children.length, 2);
  });
});
