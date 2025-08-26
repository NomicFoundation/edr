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
    assert.equal(
      parseForgeTestDuration("3m 2s 100ms 50µs 25ns"),
      3n * 60n * 1_000_000_000n + 2100050025n
    );
    assert.equal(
      parseForgeTestDuration("4h 3m 2s 100ms 50µs 25ns"),
      4n * 60n * 60n * 1_000_000_000n + 3n * 60n * 1_000_000_000n + 2100050025n
    );
  });

  it("legacy format", () => {
    assert.equal(parseForgeTestDuration({ secs: 0, nanos: 1001 }), 1001n);
    assert.equal(
      parseForgeTestDuration({ secs: 3, nanos: 1001 }),
      3n * 1_000_000_000n + 1001n
    );
    assert.equal(
      parseForgeTestDuration({ secs: 3, nanos: 0 }),
      3n * 1_000_000_000n
    );
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
    assert.throws(() => parseForgeTestDuration("123456ms"));
    assert.throws(() => parseForgeTestDuration("abc"));
    assert.throws(() => parseForgeTestDuration("123xyz"));
    assert.throws(() => parseForgeTestDuration(""));
  });

  it("unknown units should throw", () => {
    assert.throws(() => parseForgeTestDuration("123ls"), {
      message: "Unknown duration unit: ls",
    });
    assert.throws(() => parseForgeTestDuration("123ms 456ls"), {
      message: "Unknown duration unit: ls",
    });
    assert.throws(() => parseForgeTestDuration("123min"), {
      message: "Unknown duration unit: min",
    });
    assert.throws(() => parseForgeTestDuration("123j"), {
      message: "Unknown duration unit: j",
    });
  });
});
