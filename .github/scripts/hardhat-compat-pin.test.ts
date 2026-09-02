// Unit tests for hardhat-compat-pin.ts.
//
// Run with Node's built-in test runner (no extra dependencies):
//   node --test .github/scripts/hardhat-compat-pin.test.ts

import assert from "node:assert/strict";
import test from "node:test";

import { HARDHAT_PIN_PATH, parseHardhatPin } from "./hardhat-compat-pin.ts";

const SHA = "d57964b9bb2814089b26aa7d593dc222a1820848";

test("parseHardhatPin accepts a well-formed pin", () => {
  assert.deepEqual(parseHardhatPin(`{"pr":123,"sha":"${SHA}"}`), {
    pr: 123,
    sha: SHA,
  });
});

test("parseHardhatPin lowercases the sha", () => {
  const pin = parseHardhatPin(`{"pr":1,"sha":"${SHA.toUpperCase()}"}`);

  assert.equal(pin.sha, SHA);
});

test("parseHardhatPin passes other keys through", () => {
  const pin = parseHardhatPin(`{"pr":1,"sha":"${SHA}","reason":"breaking"}`);

  assert.equal(pin.reason, "breaking");
});

test("parseHardhatPin reports invalid JSON", () => {
  assert.throws(() => parseHardhatPin("not json"), /is not valid JSON/);
});

// The pin's sha is checked out by the benchmark job and its PR number is
// looked up against the Hardhat repo, so every one of these must be rejected
// rather than flowing through.
test("parseHardhatPin rejects malformed pins", () => {
  const malformed = [
    // Not an object at all. Reading `.pr` off `null` used to throw a raw
    // TypeError instead of the descriptive error.
    "null",
    "123",
    '"a string"',
    "[]",
    // Bad PR numbers.
    `{"sha":"${SHA}"}`,
    `{"pr":1.5,"sha":"${SHA}"}`,
    `{"pr":0,"sha":"${SHA}"}`,
    `{"pr":-5,"sha":"${SHA}"}`,
    `{"pr":"1","sha":"${SHA}"}`,
    // Bad shas.
    `{"pr":1}`,
    `{"pr":1,"sha":"${SHA.slice(0, 12)}"}`,
    `{"pr":1,"sha":"${SHA}aaa"}`,
    `{"pr":1,"sha":"  ${SHA}  "}`,
    `{"pr":1,"sha":"${SHA.slice(0, 39)}z"}`,
    `{"pr":1,"sha":123}`,
  ];

  for (const raw of malformed) {
    assert.throws(
      () => parseHardhatPin(raw),
      new RegExp(HARDHAT_PIN_PATH.replace(/[.]/g, "\\.")),
      `accepted a malformed pin: ${raw}`
    );
  }
});
