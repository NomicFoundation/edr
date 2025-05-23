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
      outputs: "",
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
});
