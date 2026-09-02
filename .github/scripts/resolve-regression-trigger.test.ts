// Unit tests for resolve-regression-trigger.ts.
//
// Run with Node's built-in test runner (no extra dependencies):
//   node --test .github/scripts/resolve-regression-trigger.test.ts

import assert from "node:assert/strict";
import test from "node:test";

import type { WorkflowRun } from "./github-script.ts";
import {
  resolveRegressionTrigger,
  type Context,
} from "./resolve-regression-trigger.ts";

const OWNER = "NomicFoundation";
const REPO = "edr";
const FULL = `${OWNER}/${REPO}`;

// Fixtures name only the fields the resolver reads for that call; the mocks
// widen them to the full pulls.get response shape.
interface EdrPullRequest {
  head: { repo: { full_name: string }; sha: string };
}

interface HardhatPullRequest {
  merged: boolean;
  state: string;
}

// The record of side effects the module produced (outputs, logs, comments,
// reactions).
interface Captured {
  outputs: Record<string, string>;
  infos: string[];
  notices: string[];
  warnings: string[];
  comments: string[];
  reactions: string[];
}

// `pinFile` is the raw content of .github/hardhat-compat-pin.json (absent →
// repos.getContent 404s, i.e. no pin). `hardhatPr` is the Hardhat PR that a
// valid pin's pulls.get resolves to.
function makeDeps({
  eventName,
  sha = "",
  payload = {},
  ci,
  pr,
  pinFile,
  hardhatPr,
  getContentError,
  failComments = false,
}: {
  eventName: string;
  sha?: string;
  payload?: Context["payload"];
  ci?: WorkflowRun;
  pr?: EdrPullRequest;
  pinFile?: string;
  hardhatPr?: HardhatPullRequest;
  getContentError?: unknown;
  failComments?: boolean;
}) {
  const captured: Captured = {
    outputs: {},
    infos: [],
    notices: [],
    warnings: [],
    comments: [],
    reactions: [],
  };

  const core = {
    setOutput: (name: string, value: string) => {
      captured.outputs[name] = value;
    },
    info: (message: string) => captured.infos.push(message),
    notice: (message: string) => captured.notices.push(message),
    warning: (message: string) => captured.warnings.push(message),
  };

  const github = {
    rest: {
      actions: {
        // Asserts the gate queries EDR's own CI for the PR head: querying the
        // wrong workflow or the wrong sha would still look green.
        listWorkflowRuns: async ({
          workflow_id,
          head_sha,
        }: {
          workflow_id: string;
          head_sha: string;
        }) => {
          assert.equal(workflow_id, "edr-ci.yml");
          assert.equal(head_sha, pr?.head.sha);
          return { data: { workflow_runs: ci === undefined ? [] : [ci] } };
        },
      },
      pulls: {
        get: async ({
          repo: pullRepo,
          pull_number: pullNumber,
        }: {
          repo: string;
          pull_number: number;
        }) => {
          if (pullRepo === "hardhat") {
            if (hardhatPr === undefined) {
              throw new Error(
                "unexpected pulls.get for the Hardhat repo: this test did " +
                  "not provide a `hardhatPr` fixture"
              );
            }
            assert.equal(pullNumber, PIN_PR);
            return { data: { ...hardhatPr, head: HARDHAT_HEAD } };
          }
          if (pr === undefined) {
            throw new Error(
              "unexpected pulls.get for the EDR repo: this test did not " +
                "provide a `pr` fixture"
            );
          }
          return { data: { merged: false, state: "open", ...pr } };
        },
      },
      repos: {
        // Asserts the pin is read from the pushed/PR-head sha, not from main:
        // a PR that adds a pin must have its own pin honoured.
        getContent: async ({ path, ref }: { path: string; ref: string }) => {
          assert.equal(path, ".github/hardhat-compat-pin.json");
          assert.equal(ref, pr === undefined ? sha : pr.head.sha);
          if (getContentError !== undefined) {
            throw getContentError;
          }
          if (pinFile === undefined) {
            throw Object.assign(new Error("Not Found"), { status: 404 });
          }
          return {
            data: { content: Buffer.from(pinFile).toString("base64") },
          };
        },
      },
      issues: {
        createComment: async ({ body }: { body: string }) => {
          if (failComments) {
            throw new Error("Resource not accessible by integration");
          }
          return captured.comments.push(body);
        },
      },
      reactions: {
        createForIssueComment: async ({ content }: { content: string }) =>
          captured.reactions.push(content),
      },
    },
  };

  const context = {
    repo: { owner: OWNER, repo: REPO },
    eventName,
    sha,
    serverUrl: "https://github.com",
    runId: 123,
    payload,
  };

  return { github, context, core, captured };
}

// Stand-in head for the pinned Hardhat PR; the resolver never reads it.
const HARDHAT_HEAD = {
  repo: { full_name: "NomicFoundation/hardhat" },
  sha: "",
};

// A `/bench` comment on a same-repo PR, by an authorized author.
function commentPayload(
  body: string,
  { assoc = "MEMBER", number = 7 }: { assoc?: string; number?: number } = {}
) {
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

// The first of a captured message list. Asserts one exists, so the regex
// assertions below report "none was emitted" rather than a type error.
function first(messages: string[], what: string): string {
  const message = messages[0];

  assert.ok(message !== undefined, `expected a ${what} to be emitted`);

  return message;
}

function firstComment(captured: Captured): string {
  return first(captured.comments, "status comment");
}

test("push → baseline run against Hardhat main", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "push",
    sha: "deadbeefcafe1234",
  });
  await resolveRegressionTrigger(deps);
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
  await resolveRegressionTrigger(deps);
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
  await resolveRegressionTrigger(deps);
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
  await resolveRegressionTrigger(withFilter);
  assert.equal(withFilter.captured.outputs.scenario_filter, "1inch*");
  // Explicit override wins over the default.
  assert.equal(withFilter.captured.outputs.benchmark_filter, "cold compile");

  const withoutFilter = makeDeps({
    eventName: "workflow_dispatch",
    sha: "abc123",
    payload: { inputs: {} },
  });
  await resolveRegressionTrigger(withoutFilter);
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
  await resolveRegressionTrigger(deps);
  assert.equal(captured.outputs.benchmark_filter, "*");
});

test("issue_comment → unauthorized author does not run", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench", { assoc: "NONE" }),
  });
  await resolveRegressionTrigger(deps);
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
  await resolveRegressionTrigger(deps);
  assert.equal(captured.outputs.should_run, "false");
  assert.equal(captured.comments.length, 1);
  assert.match(firstComment(captured), /can only run for branches in/);
});

test("issue_comment → same-repo PR with green CI runs and parses hardhat-ref", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench hardhat-ref=feature/x"),
    pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
    ci: { id: 1, status: "completed", conclusion: "success" },
  });
  await resolveRegressionTrigger(deps);
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
  assert.match(firstComment(captured), /Starting regression benchmark/);
  // A `*` (all) scenario filter is not called out; the benchmark default is.
  assert.doesNotMatch(firstComment(captured), /projects matching/);
  assert.match(firstComment(captured), /benchmarks matching/);
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
  await resolveRegressionTrigger(deps);
  assert.equal(captured.outputs.should_run, "true");
  assert.equal(
    captured.outputs.hardhat_ref,
    "edr-benchmark/command-step-filters"
  );
  assert.equal(captured.outputs.scenario_filter, "1inch*");
  assert.equal(captured.outputs.benchmark_filter, "test solidity");
  assert.match(firstComment(captured), /projects matching/);
  assert.match(firstComment(captured), /benchmarks matching/);
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
  await resolveRegressionTrigger(deps);
  assert.equal(captured.outputs.should_run, "true");
  assert.equal(captured.outputs.hardhat_ref, "main");
  // Quoted values preserve spaces and internal commas.
  assert.equal(captured.outputs.benchmark_filter, "warm compile,test *");
  assert.match(firstComment(captured), /benchmarks matching/);
});

test("issue_comment → parses an unquoted single-token filter", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench benchmarks=cold-compile"),
    pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
    ci: { id: 1, status: "completed", conclusion: "success" },
  });
  await resolveRegressionTrigger(deps);
  assert.equal(captured.outputs.benchmark_filter, "cold-compile");
  // No scenarios= given → default `*` (all projects).
  assert.equal(captured.outputs.scenario_filter, "*");
});

// ---------------------------------------------------------------------------
// Hardhat compat pin (.github/hardhat-compat-pin.json)
// ---------------------------------------------------------------------------

const PIN_SHA = "a".repeat(40);
const PIN_PR = 5678;
const PIN_FILE = JSON.stringify({
  pr: PIN_PR,
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
  await resolveRegressionTrigger(deps);
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
  await resolveRegressionTrigger(deps);
  assert.equal(captured.outputs.hardhat_ref, "main");
  assert.equal(captured.notices.length, 1);
  assert.match(first(captured.notices, "notice"), /can now be removed/);
});

test("push → pin PR closed without merging reverts to main with a warning", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "push",
    sha: "deadbeefcafe1234",
    pinFile: PIN_FILE,
    hardhatPr: { state: "closed", merged: false },
  });
  await resolveRegressionTrigger(deps);
  assert.equal(captured.outputs.hardhat_ref, "main");
  assert.equal(captured.warnings.length, 1);
  assert.match(first(captured.warnings, "warning"), /closed without merging/);
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
    await assert.rejects(
      resolveRegressionTrigger(deps),
      /hardhat-compat-pin\.json/
    );
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
  await resolveRegressionTrigger(deps);
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
  await resolveRegressionTrigger(deps);
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
  await resolveRegressionTrigger(deps);
  assert.equal(captured.outputs.should_run, "true");
  assert.equal(captured.outputs.hardhat_ref, PIN_SHA);
  assert.match(
    firstComment(captured),
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
  await resolveRegressionTrigger(deps);
  assert.equal(captured.outputs.should_run, "true");
  assert.equal(captured.outputs.hardhat_ref, "feature/x");
  assert.doesNotMatch(firstComment(captured), /compat pin/);
});

test("issue_comment → malformed pin posts an error comment and skips the run", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench"),
    pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
    ci: { id: 1, status: "completed", conclusion: "success" },
    pinFile: "not json",
  });
  await resolveRegressionTrigger(deps);
  assert.equal(captured.outputs.should_run, "false");
  assert.equal(captured.comments.length, 1);
  assert.match(
    firstComment(captured),
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
  await resolveRegressionTrigger(deps);
  assert.equal(captured.outputs.should_run, "false");
  assert.equal(captured.outputs.hardhat_ref, "main"); // no hardhat-ref= in body
  assert.equal(captured.comments.length, 1);
  assert.match(firstComment(captured), /hasn't passed yet/);
});

test("issue_comment → malformed payload throws", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: {},
  });
  await assert.rejects(
    resolveRegressionTrigger(deps),
    /Malformed issue_comment payload/
  );
  assert.deepEqual(captured.outputs, {});
});

test("issue_comment → every authorized association may trigger", async () => {
  for (const assoc of ["OWNER", "MEMBER", "COLLABORATOR"]) {
    const { captured, ...deps } = makeDeps({
      eventName: "issue_comment",
      payload: commentPayload("/bench hardhat-ref=main", { assoc }),
      pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
      ci: { id: 1, status: "completed", conclusion: "success" },
    });
    await resolveRegressionTrigger(deps);
    assert.equal(captured.outputs.should_run, "true", `${assoc} was refused`);
  }
});

test("issue_comment → no other association may trigger", async () => {
  for (const assoc of ["CONTRIBUTOR", "FIRST_TIME_CONTRIBUTOR", "NONE", ""]) {
    const { captured, ...deps } = makeDeps({
      eventName: "issue_comment",
      payload: commentPayload("/bench", { assoc }),
    });
    await resolveRegressionTrigger(deps);
    assert.equal(captured.outputs.should_run, "false", `${assoc} was allowed`);
  }
});

// The gating decision must survive a failing GitHub API call: the token may
// lack `issues: write`, and that must not fail the job.
test("issue_comment → a failing status comment does not fail the run", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "issue_comment",
    payload: commentPayload("/bench hardhat-ref=main"),
    pr: { head: { repo: { full_name: FULL }, sha: "1234567890ab" } },
    ci: { id: 1, status: "completed", conclusion: "success" },
    failComments: true,
  });
  await resolveRegressionTrigger(deps);
  assert.equal(captured.outputs.should_run, "true");
  assert.deepEqual(captured.comments, []);
  assert.ok(
    captured.warnings.some((w) => w.includes("Posting status comment failed")),
    `expected a warning, got: ${JSON.stringify(captured.warnings)}`
  );
});

// A pin that can't be read is NOT the same as "no pin". Falling back to main
// would silently benchmark the incompatible state the pin exists to avoid.
test("push → an unreadable pin fails loudly instead of falling back to main", async () => {
  const { captured, ...deps } = makeDeps({
    eventName: "push",
    sha: "deadbeefcafe1234",
    getContentError: Object.assign(new Error("Server Error"), { status: 500 }),
  });

  await assert.rejects(resolveRegressionTrigger(deps), /Server Error/);
  assert.deepEqual(captured.outputs, {});
});
