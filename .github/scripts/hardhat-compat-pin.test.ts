// Unit tests for hardhat-compat-pin.ts.
//
// Run with Node's built-in test runner (no extra dependencies):
//   node --test .github/scripts/hardhat-compat-pin.test.ts

import assert from "node:assert/strict";
import { writeFileSync } from "node:fs";
import test from "node:test";

import { PIN_FILE, PIN_PR, PIN_REASON, PIN_SHA, tempPinPath } from "./fakes.ts";
import type { PullRequest } from "./github-script.ts";
import {
  HARDHAT_PIN_PATH,
  parseHardhatPin,
  readHardhatPin,
  resolveHardhatRef,
} from "./hardhat-compat-pin.ts";

const SHA = "d57964b9bb2814089b26aa7d593dc222a1820848";

// Every rejection names the pin file, so the operator knows what to fix.
const PIN_PATH_PATTERN = new RegExp(HARDHAT_PIN_PATH.replace(/[.]/g, "\\."));

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
      PIN_PATH_PATTERN,
      `accepted a malformed pin: ${raw}`
    );
  }
});

// ---------------------------------------------------------------------------
// readHardhatPin
// ---------------------------------------------------------------------------

test("readHardhatPin returns the file contents", () => {
  const pinPath = tempPinPath();
  writeFileSync(pinPath, PIN_FILE);

  assert.equal(readHardhatPin(pinPath), PIN_FILE);
});

test("readHardhatPin returns undefined for a missing file", () => {
  assert.equal(readHardhatPin(tempPinPath()), undefined);
});

test("readHardhatPin fails on an unreadable pin instead of reporting no pin", () => {
  assert.throws(() => readHardhatPin(tempPinPath({ unreadable: true })));
});

// ---------------------------------------------------------------------------
// resolveHardhatRef
// ---------------------------------------------------------------------------

interface Captured {
  infos: string[];
  notices: string[];
  warnings: string[];
  requestedPrNumbers: number[];
}

// `hardhatPr` is what pulls.get resolves to for the pinned PR. `getError`
// makes it reject instead. A test that provides neither asserts that pulls.get
// is never reached.
function makeDeps({
  hardhatPr,
  getError,
}: {
  hardhatPr?: PullRequest;
  getError?: unknown;
} = {}) {
  const captured: Captured = {
    infos: [],
    notices: [],
    warnings: [],
    requestedPrNumbers: [],
  };
  const core = {
    info: (m: string) => captured.infos.push(m),
    notice: (m: string) => captured.notices.push(m),
    warning: (m: string) => captured.warnings.push(m),
  };
  const github = {
    rest: {
      pulls: {
        get: async ({
          owner,
          repo,
          pull_number: pullNumber,
        }: {
          owner: string;
          repo: string;
          pull_number: number;
        }) => {
          assert.equal(`${owner}/${repo}`, "NomicFoundation/hardhat");
          captured.requestedPrNumbers.push(pullNumber);
          if (getError !== undefined) throw getError;
          if (hardhatPr === undefined) {
            throw new Error(
              "unexpected pulls.get: this test did not provide a `hardhatPr` fixture"
            );
          }
          return { data: hardhatPr };
        },
      },
    },
  };
  return { github, core, captured };
}

const HEAD = { sha: "b".repeat(40) };

test("resolveHardhatRef without a pin resolves to main", async () => {
  const { captured, ...deps } = makeDeps();

  assert.deepEqual(await resolveHardhatRef({ ...deps, raw: undefined }), {
    ref: "main",
  });
  assert.deepEqual(captured.requestedPrNumbers, []);
});

test("resolveHardhatRef uses the sha of an open pin PR and returns the PR", async () => {
  const hardhatPr = { state: "open", merged: false, head: HEAD };
  const { captured, ...deps } = makeDeps({ hardhatPr });

  assert.deepEqual(await resolveHardhatRef({ ...deps, raw: PIN_FILE }), {
    ref: PIN_SHA,
    pin: { pr: PIN_PR, sha: PIN_SHA, reason: PIN_REASON },
    hardhatPr,
  });
  assert.deepEqual(captured.requestedPrNumbers, [PIN_PR]);
  assert.match(captured.infos.join("\n"), /compat pin active/);
});

test("resolveHardhatRef reverts to main with a notice once the pin PR is merged", async () => {
  const { captured, ...deps } = makeDeps({
    hardhatPr: { state: "closed", merged: true, head: HEAD },
  });

  assert.deepEqual(await resolveHardhatRef({ ...deps, raw: PIN_FILE }), {
    ref: "main",
  });
  assert.equal(captured.notices.length, 1);
  assert.match(captured.notices.join("\n"), /can now be removed/);
  assert.deepEqual(captured.warnings, []);
});

test("resolveHardhatRef reverts to main with a warning when the pin PR was closed without merging", async () => {
  const { captured, ...deps } = makeDeps({
    hardhatPr: { state: "closed", merged: false, head: HEAD },
  });

  assert.deepEqual(await resolveHardhatRef({ ...deps, raw: PIN_FILE }), {
    ref: "main",
  });
  assert.equal(captured.warnings.length, 1);
  assert.match(captured.warnings.join("\n"), /closed without merging/);
  assert.deepEqual(captured.notices, []);
});

test("resolveHardhatRef rejects a malformed pin before looking up the PR", async () => {
  const { captured, ...deps } = makeDeps();

  await assert.rejects(
    resolveHardhatRef({ ...deps, raw: "not json" }),
    PIN_PATH_PATTERN
  );
  assert.deepEqual(captured.requestedPrNumbers, []);
});

test("resolveHardhatRef names the pin file when the pinned PR does not exist", async () => {
  const { captured: _, ...deps } = makeDeps({
    getError: Object.assign(new Error("Not Found"), { status: 404 }),
  });

  await assert.rejects(
    resolveHardhatRef({ ...deps, raw: PIN_FILE }),
    new RegExp(`${PIN_PATH_PATTERN.source}.*#${PIN_PR} not found`)
  );
});

test("resolveHardhatRef propagates any other failed PR lookup", async () => {
  const { captured, ...deps } = makeDeps({
    getError: Object.assign(new Error("Server Error"), { status: 500 }),
  });

  await assert.rejects(
    resolveHardhatRef({ ...deps, raw: PIN_FILE }),
    /Server Error/
  );
  assert.deepEqual(captured.requestedPrNumbers, [PIN_PR]);
  assert.deepEqual(captured.notices, []);
  assert.deepEqual(captured.warnings, []);
});
