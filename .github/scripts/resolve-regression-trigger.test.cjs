// Unit tests for resolve-regression-trigger.cjs.
//
// Run with Node's built-in test runner (no extra dependencies):
//   node --test .github/scripts/

const test = require("node:test");
const assert = require("node:assert/strict");

const resolve = require("./resolve-regression-trigger.cjs");

const OWNER = "NomicFoundation";
const REPO = "edr";
const FULL = `${OWNER}/${REPO}`;

// Build a mocked { github, context, core } plus a `captured` record of the
// side effects the module produced (outputs, logs, comments, reactions).
//
// `pinFile` is the raw content of .github/hardhat-compat-pin.json (absent →
// repos.getContent 404s, i.e. no pin). `hardhatPr` is the Hardhat PR that a
// valid pin's pulls.get resolves to.
function makeDeps({
  eventName,
  sha,
  payload = {},
  ci,
  pr,
  pinFile,
  hardhatPr,
} = {}) {
  const captured = {
    outputs: {},
    infos: [],
    notices: [],
    warnings: [],
    comments: [],
    reactions: [],
  };

  const core = {
    setOutput: (k, v) => {
      captured.outputs[k] = v;
    },
    info: (m) => captured.infos.push(m),
    notice: (m) => captured.notices.push(m),
    warning: (m) => captured.warnings.push(m),
  };

  const github = {
    rest: {
      actions: {
        listWorkflowRuns: async () => ({
          data: { workflow_runs: ci === undefined ? [] : [ci] },
        }),
      },
      pulls: {
        get: async ({ repo: pullRepo }) => {
          if (pullRepo === "hardhat") {
            if (hardhatPr === undefined)
              throw new Error("hardhat pulls.get not expected");
            return { data: hardhatPr };
          }
          if (pr === undefined) throw new Error("pulls.get not expected");
          return { data: pr };
        },
      },
      repos: {
        getContent: async () => {
          if (pinFile === undefined) {
            const e = new Error("Not Found");
            e.status = 404;
            throw e;
          }
          return {
            data: { content: Buffer.from(pinFile).toString("base64") },
          };
        },
      },
      issues: {
        createComment: async ({ body }) => captured.comments.push(body),
      },
      reactions: {
        createForIssueComment: async ({ content }) =>
          captured.reactions.push(content),
      },
    },
  };

  const context = {
    repo: { owner: OWNER, repo: REPO },
    eventName,
    sha,
    payload,
  };

  return { github, context, core, captured };
}

// A `/bench` comment on a same-repo PR, by an authorized author.
function commentPayload(body, { assoc = "MEMBER", number = 7 } = {}) {
  return {
    comment: {
      author_association: assoc,
      user: { login: "dev" },
      id: 99,
      body,
    },
    issue: { number },
  };
}

test("push → baseline run against Hardhat main", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "push",
    sha: "deadbeefcafe1234",
  });
  await resolve(deps);
  assert.deepEqual(captured.outputs, {
    should_run: "true",
    edr_ref: "deadbeefcafe1234",
    hardhat_ref: "main",
    is_baseline: "true",
    // Baseline runs all projects (`*`), but only the default test-execution
    // benchmarks — EDR doesn't affect compilation, so compile ones are skipped.
    scenario_filter: "*",
    benchmark_filter: "test solidity,test mocha,test vitest",
  });
});

test("workflow_dispatch → uses the requested hardhat-ref", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "workflow_dispatch",
    sha: "abc123",
    payload: { inputs: { "hardhat-ref": "v-next" } },
  });
  await resolve(deps);
  assert.equal(captured.outputs.should_run, "true");
  assert.equal(captured.outputs.hardhat_ref, "v-next");
  assert.equal(captured.outputs.is_baseline, "false");
});

test("workflow_dispatch → defaults hardhat-ref to main", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "workflow_dispatch",
    sha: "abc123",
    payload: { inputs: {} },
  });
  await resolve(deps);
  assert.equal(captured.outputs.hardhat_ref, "main");
});

test("workflow_dispatch → forwards explicit filters; benchmark uses the default when unset", async () => {
  const withFilter = makeDeps({
    eventName: "workflow_dispatch",
    sha: "abc123",
    payload: {
      inputs: {
        "scenario-filter": "1inch*",
        "benchmark-filter": "cold compile",
      },
    },
  });
  await resolve(withFilter);
  assert.equal(withFilter.captured.outputs.scenario_filter, "1inch*");
  // Explicit override wins over the default.
  assert.equal(withFilter.captured.outputs.benchmark_filter, "cold compile");

  const withoutFilter = makeDeps({
    eventName: "workflow_dispatch",
    sha: "abc123",
    payload: { inputs: {} },
  });
  await resolve(withoutFilter);
  // No scenario-filter given → default `*` (all projects).
  assert.equal(withoutFilter.captured.outputs.scenario_filter, "*");
  // No benchmark-filter given → default test-execution benchmarks.
  assert.equal(
    withoutFilter.captured.outputs.benchmark_filter,
    "test solidity,test mocha,test vitest"
  );
});

test("workflow_dispatch → benchmark-filter=* runs the full suite", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "workflow_dispatch",
    sha: "abc123",
    payload: { inputs: { "benchmark-filter": "*" } },
  });
  await resolve(deps);
  assert.equal(captured.outputs.benchmark_filter, "*");
});

test("issue_comment → unauthorized author does not run", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench", { assoc: "NONE" }),
  });
  await resolve(deps);
  assert.equal(captured.outputs.should_run, "false");
  assert.equal(captured.warnings.length, 1);
  assert.deepEqual(captured.reactions, ["eyes"]); // request acknowledged
  assert.deepEqual(captured.comments, []); // but nothing posted
});

test("issue_comment → fork PR is rejected", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench"),
    pr: { head: { repo: { full_name: "attacker/edr" }, sha: "f0f0f0" } },
  });
  await resolve(deps);
  assert.equal(captured.outputs.should_run, "false");
  assert.equal(captured.comments.length, 1);
  assert.match(captured.comments[0], /can only run for branches in/);
});

test("issue_comment → same-repo PR with green CI runs and parses hardhat-ref", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench hardhat-ref=feature/x"),
    pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
    ci: { id: 1, status: "completed", conclusion: "success" },
  });
  await resolve(deps);
  assert.equal(captured.outputs.should_run, "true");
  assert.equal(captured.outputs.edr_ref, "1234567890ab");
  assert.equal(captured.outputs.hardhat_ref, "feature/x");
  assert.equal(captured.outputs.is_baseline, "false");
  assert.equal(captured.outputs.scenario_filter, "*"); // no scenarios= → all
  // default test-execution benchmarks (no benchmarks= in body)
  assert.equal(
    captured.outputs.benchmark_filter,
    "test solidity,test mocha,test vitest"
  );
  assert.equal(captured.comments.length, 1);
  assert.match(captured.comments[0], /Starting regression benchmark/);
  // A `*` (all) scenario filter is not called out; the benchmark default is.
  assert.doesNotMatch(captured.comments[0], /projects matching/);
  assert.match(captured.comments[0], /benchmarks matching/);
});

test("issue_comment → parses the 1inch* / test solidity example against a hardhat ref", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload(
      '/bench hardhat-ref=edr-benchmark/command-step-filters scenarios=1inch* benchmarks="test solidity"'
    ),
    pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
    ci: { id: 1, status: "completed", conclusion: "success" },
  });
  await resolve(deps);
  assert.equal(captured.outputs.should_run, "true");
  assert.equal(
    captured.outputs.hardhat_ref,
    "edr-benchmark/command-step-filters"
  );
  assert.equal(captured.outputs.scenario_filter, "1inch*");
  assert.equal(captured.outputs.benchmark_filter, "test solidity");
  assert.match(captured.comments[0], /projects matching/);
  assert.match(captured.comments[0], /benchmarks matching/);
});

test("issue_comment → parses a quoted benchmarks= glob (spaces + commas preserved)", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload(
      '/bench benchmarks="warm compile,test *" hardhat-ref=main'
    ),
    pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
    ci: { id: 1, status: "completed", conclusion: "success" },
  });
  await resolve(deps);
  assert.equal(captured.outputs.should_run, "true");
  assert.equal(captured.outputs.hardhat_ref, "main");
  // Quoted values preserve spaces and internal commas.
  assert.equal(captured.outputs.benchmark_filter, "warm compile,test *");
  assert.match(captured.comments[0], /benchmarks matching/);
});

test("issue_comment → parses an unquoted single-token filter", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench benchmarks=cold-compile"),
    pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
    ci: { id: 1, status: "completed", conclusion: "success" },
  });
  await resolve(deps);
  assert.equal(captured.outputs.benchmark_filter, "cold-compile");
  // No scenarios= given → default `*` (all projects).
  assert.equal(captured.outputs.scenario_filter, "*");
});

// ---------------------------------------------------------------------------
// Hardhat compat pin (.github/hardhat-compat-pin.json)
// ---------------------------------------------------------------------------

const PIN_SHA = "a".repeat(40);
const PIN_FILE = JSON.stringify({
  pr: 5678,
  sha: PIN_SHA,
  reason: "needs hardhat counterpart",
});

test("push → open pin PR pins the Hardhat ref", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "push",
    sha: "deadbeefcafe1234",
    pinFile: PIN_FILE,
    hardhatPr: { state: "open", merged: false },
  });
  await resolve(deps);
  assert.equal(captured.outputs.should_run, "true");
  assert.equal(captured.outputs.hardhat_ref, PIN_SHA);
  assert.equal(captured.outputs.is_baseline, "true");
});

test("push → merged pin PR reverts to main and suggests removing the pin", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "push",
    sha: "deadbeefcafe1234",
    pinFile: PIN_FILE,
    hardhatPr: { state: "closed", merged: true },
  });
  await resolve(deps);
  assert.equal(captured.outputs.hardhat_ref, "main");
  assert.equal(captured.notices.length, 1);
  assert.match(captured.notices[0], /can now be removed/);
});

test("push → pin PR closed without merging reverts to main with a warning", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "push",
    sha: "deadbeefcafe1234",
    pinFile: PIN_FILE,
    hardhatPr: { state: "closed", merged: false },
  });
  await resolve(deps);
  assert.equal(captured.outputs.hardhat_ref, "main");
  assert.equal(captured.warnings.length, 1);
  assert.match(captured.warnings[0], /closed without merging/);
});

test("push → malformed pin fails the run loudly", async () => {
  for (const pinFile of [
    "not json",
    JSON.stringify({ pr: 5678 }), // missing sha
    JSON.stringify({ pr: 5678, sha: "abc123" }), // short sha
    JSON.stringify({ pr: "5678", sha: PIN_SHA }), // pr not a number
  ]) {
    const { captured, ...deps } = makeDeps({
      eventName: "push",
      sha: "deadbeefcafe1234",
      pinFile,
    });
    await assert.rejects(resolve(deps), /hardhat-compat-pin\.json/);
  }
});

test("workflow_dispatch → empty hardhat-ref uses an open pin", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "workflow_dispatch",
    sha: "abc123",
    payload: { inputs: {} },
    pinFile: PIN_FILE,
    hardhatPr: { state: "open", merged: false },
  });
  await resolve(deps);
  assert.equal(captured.outputs.hardhat_ref, PIN_SHA);
  assert.equal(captured.outputs.is_baseline, "false");
});

test("workflow_dispatch → explicit hardhat-ref wins over the pin", async () => {
  // No `hardhatPr` in the mock: resolving the pin would throw, proving the
  // pin file isn't even consulted when an explicit ref is given.
  const { captured, ...deps } = makeDeps({
    eventName: "workflow_dispatch",
    sha: "abc123",
    payload: { inputs: { "hardhat-ref": "v-next" } },
    pinFile: PIN_FILE,
  });
  await resolve(deps);
  assert.equal(captured.outputs.hardhat_ref, "v-next");
});

test("issue_comment → `/bench` without hardhat-ref uses an open pin and says so", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench"),
    pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
    ci: { id: 1, status: "completed", conclusion: "success" },
    pinFile: PIN_FILE,
    hardhatPr: { state: "open", merged: false },
  });
  await resolve(deps);
  assert.equal(captured.outputs.should_run, "true");
  assert.equal(captured.outputs.hardhat_ref, PIN_SHA);
  assert.match(
    captured.comments[0],
    /compat pin for NomicFoundation\/hardhat#5678/
  );
});

test("issue_comment → explicit hardhat-ref= wins over the pin", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench hardhat-ref=feature/x"),
    pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
    ci: { id: 1, status: "completed", conclusion: "success" },
    pinFile: PIN_FILE,
  });
  await resolve(deps);
  assert.equal(captured.outputs.should_run, "true");
  assert.equal(captured.outputs.hardhat_ref, "feature/x");
  assert.doesNotMatch(captured.comments[0], /compat pin/);
});

test("issue_comment → malformed pin posts an error comment and skips the run", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench"),
    pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
    ci: { id: 1, status: "completed", conclusion: "success" },
    pinFile: "not json",
  });
  await resolve(deps);
  assert.equal(captured.outputs.should_run, "false");
  assert.equal(captured.comments.length, 1);
  assert.match(
    captured.comments[0],
    /Could not resolve the Hardhat compat pin/
  );
});

test("issue_comment → same-repo PR with failing CI does not run", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench"),
    pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
    ci: { id: 1, status: "completed", conclusion: "failure" },
  });
  await resolve(deps);
  assert.equal(captured.outputs.should_run, "false");
  assert.equal(captured.outputs.hardhat_ref, "main"); // no hardhat-ref= in body
  assert.equal(captured.comments.length, 1);
  assert.match(captured.comments[0], /hasn't passed yet/);
});
