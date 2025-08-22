import assert from "node:assert/strict";
import { it, describe } from "node:test";
import { parseForgeTestDuration } from "../src/solidity-tests.js";

describe("parseForgeTestDuration", () => {
  it("valid single units", () => {
    assert.equal(parseForgeTestDuration("5ms"), 5000000n);
    assert.equal(parseForgeTestDuration("287µs"), 287000n);
    assert.equal(parseForgeTestDuration("747ns"), 747n);
    assert.equal(parseForgeTestDuration("2s"), 2000000000n);
  });

  it("valid multiple units", () => {
    assert.equal(parseForgeTestDuration("5ms 287µs 747ns"), 5287747n);
    assert.equal(parseForgeTestDuration("1s 500ms"), 1500000000n);
    assert.equal(parseForgeTestDuration("2s 100ms 50µs 25ns"), 2100050025n);
  });

  it("zero values", () => {
    assert.equal(parseForgeTestDuration("0ns"), 0n);
    assert.equal(parseForgeTestDuration("0ms"), 0n);
    assert.equal(parseForgeTestDuration("0µs"), 0n);
    assert.equal(parseForgeTestDuration("0s"), 0n);
    assert.equal(parseForgeTestDuration("0ms 0µs 0ns"), 0n);
  });

  it("invalid formats should throw", () => {
    assert.throws(() => parseForgeTestDuration("123"));
    assert.throws(() => parseForgeTestDuration("123 456ms"));
    assert.throws(() => parseForgeTestDuration("abc"));
    assert.throws(() => parseForgeTestDuration("123xyz"));
    assert.throws(() => parseForgeTestDuration(""));
  });

  it("unknown units should throw", () => {
    assert.throws(() => parseForgeTestDuration("123us"), {
      message: "Unknown duration unit: us",
    });
    assert.throws(() => parseForgeTestDuration("123ms 456us"), {
      message: "Unknown duration unit: us",
    });
    assert.throws(() => parseForgeTestDuration("123min"), {
      message: "Unknown duration unit: min",
    });
    assert.throws(() => parseForgeTestDuration("123h"), {
      message: "Unknown duration unit: h",
    });
  });
});
