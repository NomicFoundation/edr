import assert from "node:assert/strict";
import { before, describe, it } from "node:test";
import { TestContext } from "./testContext.js";
import { ShowTraces, CallKind, LogKind, CallTrace } from "@ignored/edr";

describe("Call traces", () => {
  let testCallTraces: Map<string, CallTrace[]>;

  before(async () => {
    const testContext = await TestContext.setup();
    const runResult =
      await testContext.runTestsWithStats("CallTraces", { traces: ShowTraces.All });
    testCallTraces = runResult.callTraces;
  });

  it("no children", async function () {
    const trace = testCallTraces.get("testNoChildren()");
    assert.equal(trace?.length, 1);
    assert.deepEqual(trace[0], {
      kind: CallKind.Call,
      success: true,
      cheatcode: false,
      gasUsed: trace[0].gasUsed, // avoid coupling test to specific gas costs
      value: 0n,
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
    assert.deepEqual(child.inputs, { name: "childCall", arguments: [] });
  });

  it("single event", async function () {
    const trace = testCallTraces.get("testSingleEvent()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);
    const event = trace[0].children[0];
    assert.equal(event.kind, LogKind.Log);
    assert.deepEqual(event.parameters, { name: "OneEvent", arguments: ["x: 123"] });
  });

  it("multiple children", async function () {
    const trace = testCallTraces.get("testManyChildren()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 5);

    const child0 = trace[0].children[0];
    assert.equal(child0.kind, LogKind.Log);
    assert.deepEqual(child0.parameters, { name: "OneEvent", arguments: ["x: 1"] });

    const child1 = trace[0].children[1];
    assert.equal(child1.kind, CallKind.Call);
    assert.deepEqual(child1.inputs, { name: "childCall", arguments: [] });

    const child2 = trace[0].children[2];
    assert.equal(child2.kind, LogKind.Log);
    assert.deepEqual(child2.parameters, { name: "OneEvent", arguments: ["x: 2"] });

    const child3 = trace[0].children[3];
    assert.equal(child3.kind, CallKind.Call);
    assert.deepEqual(child3.inputs, { name: "childCall", arguments: [] });

    const child4 = trace[0].children[4];
    assert.equal(child4.kind, LogKind.Log);
    assert.deepEqual(child4.parameters, { name: "OneEvent", arguments: ["x: 3"] });
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
    assert.deepEqual(grandChild.inputs, { name: "childCall", arguments: [] });
  });

  it("call with value", async function () {
    const trace = testCallTraces.get("testCallWithValue()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 2);

    // First child is vm.deal cheatcode
    const dealCall = trace[0].children[0];
    assert.equal(dealCall.kind, CallKind.Call);
    assert.equal(dealCall.cheatcode, true);

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
    assert.equal(child.cheatcode, true);
    assert.deepEqual(child.inputs, { name: "addr", arguments: ["<pk>"] });
  });

  it.skip("labeled address", async function () {
    const trace = testCallTraces.get("testLabelAddress()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 2);

    const labelCall = trace[0].children[0];
    assert.equal(labelCall.kind, CallKind.Call);
    assert.equal(labelCall.cheatcode, true);

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
    assert.ok(child.inputs instanceof Uint8Array);
    assert.deepEqual(child.inputs, new Uint8Array([0xde, 0xad, 0xbe, 0xef]));
  });

  it("undecoded outputs", async function () {
    const trace = testCallTraces.get("testUndecodedOutputs()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);

    const child = trace[0].children[0];
    assert.equal(child.kind, CallKind.Call);
    assert.ok(child.outputs instanceof Uint8Array);
    // Should contain the raw bytes returned by the assembly code
    assert.ok(child.outputs.length > 0);
  });

  it("anonymous event", async function () {
    const trace = testCallTraces.get("testAnonymousEvent()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);

    const event = trace[0].children[0];
    assert.equal(event.kind, LogKind.Log);
    // TODO: test the contents of the array
    assert.ok(Array.isArray(event.parameters));
    assert.equal(event.parameters.length, 3); // 2 indexed topics + data
  });

  it("create contract", async function () {
    const trace = testCallTraces.get("testCreateContract()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 1);

    const child = trace[0].children[0];
    assert.equal(child.kind, CallKind.Create);
    assert.equal(child.success, true);
    assert.equal(child.contract, "CreateMe");
    assert(typeof child.outputs === 'string');
    assert.match(child.outputs, /^\d+ bytes of code$/);
  });

  it("static and delegate calls", async function () {
    const trace = testCallTraces.get("testStaticAndDelegateCall()");
    assert.equal(trace?.length, 1);
    assert.equal(trace[0].children.length, 2);

    const staticCall = trace[0].children[0];
    assert.equal(staticCall.kind, CallKind.StaticCall);
    assert.deepEqual(staticCall.inputs, { name: "childCall", arguments: [] });

    const delegateCall = trace[0].children[1];
    assert.equal(delegateCall.kind, CallKind.DelegateCall);
    assert.deepEqual(delegateCall.inputs, { name: "childCall", arguments: [] });
  });
});
