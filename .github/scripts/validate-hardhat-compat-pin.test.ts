// Unit tests for validate-hardhat-compat-pin.ts.
//
// Run with Node's built-in test runner (no extra dependencies):
//   node --test .github/scripts/validate-hardhat-compat-pin.test.ts

import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import type { PullRequest } from "./github-script.ts";
import { validateHardhatCompatPin } from "./validate-hardhat-compat-pin.ts";

interface Captured {
  infos: string[];
  notices: string[];
  warnings: string[];
  compared: string[];
}

const SHA = "d57964b9bb2814089b26aa7d593dc222a1820848";
const HEAD_SHA = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

// Write `raw` (or nothing, for the missing-file case) to a temp pin file and
// build mocked { github, core } plus a `captured` record of side effects.
//
// `hardhatPr` is what pulls.get resolves to (absent → 404). `comparison` is
// what compareCommitsWithBasehead resolves to (absent → 404, i.e. unknown sha).
function makeDeps({
  raw,
  hardhatPr,
  comparison,
  unreadable = false,
}: {
  raw?: string;
  hardhatPr?: PullRequest;
  comparison?: { status: string };
  unreadable?: boolean;
} = {}) {
  const dir = mkdtempSync(join(tmpdir(), "compat-pin-"));
  // A directory in the pin's place reads as EISDIR, not ENOENT.
  const pinPath = unreadable ? dir : join(dir, "hardhat-compat-pin.json");
  if (raw !== undefined) writeFileSync(pinPath, raw);

  const captured: Captured = {
    infos: [],
    notices: [],
    warnings: [],
    compared: [],
  };
  const core = {
    info: (m: string) => captured.infos.push(m),
    notice: (m: string) => captured.notices.push(m),
    warning: (m: string) => captured.warnings.push(m),
  };

  const notFound = () => Object.assign(new Error("Not Found"), { status: 404 });

  const github = {
    rest: {
      pulls: {
        get: async () => {
          if (hardhatPr === undefined) throw notFound();
          return { data: hardhatPr };
        },
      },
      repos: {
        compareCommitsWithBasehead: async ({
          basehead,
        }: {
          basehead: string;
        }) => {
          captured.compared.push(basehead);
          if (comparison === undefined) throw notFound();
          return { data: comparison };
        },
      },
    },
  };

  return { github, core, pinPath, captured };
}

const openPr = { state: "open", merged: false, head: { sha: HEAD_SHA } };
const validRaw = JSON.stringify({ pr: 8548, sha: SHA, reason: "compat" });

test("missing pin file passes with an info message", async () => {
  const { captured, ...deps } = makeDeps();
  await validateHardhatCompatPin(deps);
  assert.match(captured.infos.join("\n"), /not present; nothing to validate/);
});

test("invalid JSON fails", async () => {
  const { captured: _, ...deps } = makeDeps({ raw: "{ oops" });
  await assert.rejects(validateHardhatCompatPin(deps), /is not valid JSON/);
});

test("malformed shape fails (truncated sha)", async () => {
  const { captured: _, ...deps } = makeDeps({
    raw: JSON.stringify({ pr: 8548, sha: SHA.slice(0, 12) }),
  });
  await assert.rejects(
    validateHardhatCompatPin(deps),
    /full 40-hex commit sha/
  );
});

test("nonexistent Hardhat PR fails", async () => {
  const { captured: _, ...deps } = makeDeps({ raw: validRaw });
  await assert.rejects(
    validateHardhatCompatPin(deps),
    /Hardhat PR .*#8548 not found/
  );
});

test("open PR with the sha on it passes", async () => {
  const { captured, ...deps } = makeDeps({
    raw: validRaw,
    hardhatPr: openPr,
    comparison: { status: "ahead" },
  });
  await validateHardhatCompatPin(deps);
  assert.deepEqual(captured.compared, [`${SHA}...${HEAD_SHA}`]);
  assert.match(captured.infos.join("\n"), /compat pin OK: .*#8548 \(open\)/);
});

test("sha is lowercased before comparing", async () => {
  const { captured, ...deps } = makeDeps({
    raw: JSON.stringify({ pr: 8548, sha: SHA.toUpperCase() }),
    hardhatPr: openPr,
    comparison: { status: "identical" },
  });
  await validateHardhatCompatPin(deps);
  assert.deepEqual(captured.compared, [`${SHA}...${HEAD_SHA}`]);
});

test("sha unknown to the Hardhat repo fails", async () => {
  const { captured: _, ...deps } = makeDeps({
    raw: validRaw,
    hardhatPr: openPr,
  });
  await assert.rejects(validateHardhatCompatPin(deps), /does not exist in/);
});

test("sha not reachable from the PR head fails", async () => {
  const { captured: _, ...deps } = makeDeps({
    raw: validRaw,
    hardhatPr: openPr,
    comparison: { status: "diverged" },
  });
  await assert.rejects(
    validateHardhatCompatPin(deps),
    /not reachable from the head .*diverged/s
  );
});

test("merged PR passes with a remove-the-pin notice, skipping the sha check", async () => {
  const { captured, ...deps } = makeDeps({
    raw: validRaw,
    hardhatPr: { state: "closed", merged: true, head: { sha: HEAD_SHA } },
  });
  await validateHardhatCompatPin(deps);
  assert.match(captured.notices.join("\n"), /already been merged/);
  // The merged branch must return, not fall through into the closed branch.
  assert.deepEqual(captured.warnings, []);
  assert.deepEqual(captured.compared, []);
});

test("PR closed without merging passes with a warning, skipping the sha check", async () => {
  const { captured, ...deps } = makeDeps({
    raw: validRaw,
    hardhatPr: { state: "closed", merged: false, head: { sha: HEAD_SHA } },
  });
  await validateHardhatCompatPin(deps);
  assert.match(captured.warnings.join("\n"), /closed without merging/);
  assert.deepEqual(captured.notices, []);
  assert.deepEqual(captured.compared, []);
});

// A pin that exists but can't be read is not the same as a missing pin:
// reporting "nothing to validate" would pass the check green over a real pin.
test("an unreadable pin file fails instead of passing as missing", async () => {
  const { captured, ...deps } = makeDeps({ unreadable: true });

  await assert.rejects(validateHardhatCompatPin(deps));
  assert.deepEqual(captured.infos, []);
});

// `reason` is free-form and unvalidated, so an empty one must not produce a
// dangling separator in the log line.
test("an empty reason is omitted from the OK line", async () => {
  const { captured, ...deps } = makeDeps({
    raw: JSON.stringify({ pr: 1, sha: SHA, reason: "" }),
    hardhatPr: { state: "open", merged: false, head: { sha: HEAD_SHA } },
    comparison: { status: "ahead" },
  });
  await validateHardhatCompatPin(deps);
  assert.match(captured.infos.join("\n"), /Hardhat compat pin OK/);
  assert.doesNotMatch(captured.infos.join("\n"), /—\s*$/m);
});
