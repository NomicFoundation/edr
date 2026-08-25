// Unit tests for validate-hardhat-compat-pin.cjs.
//
// Run with Node's built-in test runner (no extra dependencies):
//   node --test .github/scripts/

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const validate = require("./validate-hardhat-compat-pin.cjs");

const SHA = "d57964b9bb2814089b26aa7d593dc222a1820848";
const HEAD_SHA = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

// Write `raw` (or nothing, for the missing-file case) to a temp pin file and
// build mocked { github, core } plus a `captured` record of side effects.
//
// `hardhatPr` is what pulls.get resolves to (absent → 404). `comparison` is
// what compareCommitsWithBasehead resolves to (absent → 404, i.e. unknown sha).
function makeDeps({ raw, hardhatPr, comparison } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "compat-pin-"));
  const pinPath = path.join(dir, "hardhat-compat-pin.json");
  if (raw !== undefined) fs.writeFileSync(pinPath, raw);

  const captured = { infos: [], notices: [], warnings: [], compared: [] };
  const core = {
    info: (m) => captured.infos.push(m),
    notice: (m) => captured.notices.push(m),
    warning: (m) => captured.warnings.push(m),
  };

  const notFound = () => {
    const e = new Error("Not Found");
    e.status = 404;
    return e;
  };

  const github = {
    rest: {
      pulls: {
        get: async () => {
          if (hardhatPr === undefined) throw notFound();
          return { data: hardhatPr };
        },
      },
      repos: {
        compareCommitsWithBasehead: async ({ basehead }) => {
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
  await validate(deps);
  assert.match(captured.infos.join("\n"), /not present; nothing to validate/);
});

test("invalid JSON fails", async () => {
  const { captured: _, ...deps } = makeDeps({ raw: "{ oops" });
  await assert.rejects(validate(deps), /is not valid JSON/);
});

test("malformed shape fails (truncated sha)", async () => {
  const { captured: _, ...deps } = makeDeps({
    raw: JSON.stringify({ pr: 8548, sha: SHA.slice(0, 12) }),
  });
  await assert.rejects(validate(deps), /full 40-hex commit sha/);
});

test("nonexistent Hardhat PR fails", async () => {
  const { captured: _, ...deps } = makeDeps({ raw: validRaw });
  await assert.rejects(validate(deps), /Hardhat PR .*#8548 not found/);
});

test("open PR with the sha on it passes", async () => {
  const { captured, ...deps } = makeDeps({
    raw: validRaw,
    hardhatPr: openPr,
    comparison: { status: "ahead" },
  });
  await validate(deps);
  assert.deepEqual(captured.compared, [`${SHA}...${HEAD_SHA}`]);
  assert.match(captured.infos.join("\n"), /compat pin OK: .*#8548 \(open\)/);
});

test("sha is lowercased before comparing", async () => {
  const { captured, ...deps } = makeDeps({
    raw: JSON.stringify({ pr: 8548, sha: SHA.toUpperCase() }),
    hardhatPr: openPr,
    comparison: { status: "identical" },
  });
  await validate(deps);
  assert.deepEqual(captured.compared, [`${SHA}...${HEAD_SHA}`]);
});

test("sha unknown to the Hardhat repo fails", async () => {
  const { captured: _, ...deps } = makeDeps({
    raw: validRaw,
    hardhatPr: openPr,
  });
  await assert.rejects(validate(deps), /does not exist in/);
});

test("sha not reachable from the PR head fails", async () => {
  const { captured: _, ...deps } = makeDeps({
    raw: validRaw,
    hardhatPr: openPr,
    comparison: { status: "diverged" },
  });
  await assert.rejects(
    validate(deps),
    /not reachable from the head .*diverged/s
  );
});

test("merged PR passes with a remove-the-pin notice, skipping the sha check", async () => {
  const { captured, ...deps } = makeDeps({
    raw: validRaw,
    hardhatPr: { state: "closed", merged: true, head: { sha: HEAD_SHA } },
  });
  await validate(deps);
  assert.match(captured.notices.join("\n"), /already been merged/);
  assert.deepEqual(captured.compared, []);
});

test("PR closed without merging passes with a warning, skipping the sha check", async () => {
  const { captured, ...deps } = makeDeps({
    raw: validRaw,
    hardhatPr: { state: "closed", merged: false, head: { sha: HEAD_SHA } },
  });
  await validate(deps);
  assert.match(captured.warnings.join("\n"), /closed without merging/);
  assert.deepEqual(captured.compared, []);
});
