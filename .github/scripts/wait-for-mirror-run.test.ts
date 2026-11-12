// Unit tests for wait-for-mirror-run.ts.
//
// Run with Node's built-in test runner (no extra dependencies):
//   node --test .github/scripts/wait-for-mirror-run.test.ts

import assert from "node:assert/strict";
import test from "node:test";

import { fakeClock, runsOf, sequence, type RunPage } from "./fakes.ts";
import type { WorkflowRun } from "./github-script.ts";
import { waitForMirrorRun, type Context } from "./wait-for-mirror-run.ts";
import type { GitHub } from "./wait-for-workflow-run.ts";

const OWNER = "NomicFoundation";
const REPO = "edr";
const PUSH_SHA = "1e5fbd883f58b0f0b1a6c52e5cf0f7b9cf0b7a01";
const PR_HEAD_SHA = "b63ac3702506637a8c4b51a9ee1f4a0c5e2d6a19";

// `pages` is one listWorkflowRuns page per call, the last repeating; `run`
// is shorthand for a single one-run page.
function makeDeps({
  run,
  pages = [run],
  prHeadSha,
  apiError,
}: {
  run?: WorkflowRun;
  pages?: RunPage[];
  prHeadSha?: string;
  apiError?: unknown;
} = {}) {
  const nextPage = sequence(pages);
  const shas: string[] = [];
  const infos: string[] = [];
  const warnings: string[] = [];

  const github: GitHub = {
    rest: {
      actions: {
        listWorkflowRuns: async ({ workflow_id, head_sha }) => {
          assert.equal(workflow_id, "mirror-docker-images.yml");
          shas.push(head_sha);
          if (apiError !== undefined) {
            throw apiError;
          }
          return { data: { workflow_runs: runsOf(nextPage()) } };
        },
      },
    },
  };

  const context: Context = {
    repo: { owner: OWNER, repo: REPO },
    sha: PUSH_SHA,
    serverUrl: "https://github.com",
    payload:
      prHeadSha === undefined
        ? {}
        : { pull_request: { head: { sha: prHeadSha } } },
  };

  return {
    github,
    context,
    core: {
      info: (m: string) => infos.push(m),
      notice: () => {},
      warning: (m: string) => warnings.push(m),
    },
    clock: fakeClock(),
    shas,
    infos,
    warnings,
  };
}

function completed(conclusion: string | null): WorkflowRun {
  return { id: 42, status: "completed", conclusion };
}

test("no mirror run for the commit: does not wait", async () => {
  const deps = makeDeps();
  await waitForMirrorRun(deps);

  assert.deepEqual(deps.shas, [PUSH_SHA]);
  assert.match(deps.infos.join("\n"), /No mirror run for /);
  assert.deepEqual(deps.warnings, []);
});

test("a successful mirror run is reported with its URL", async () => {
  const deps = makeDeps({ run: completed("success") });
  await waitForMirrorRun(deps);

  assert.equal(
    deps.infos[0],
    "Mirror run succeeded: https://github.com/NomicFoundation/edr/actions/runs/42"
  );
  assert.deepEqual(deps.warnings, []);
});

test("a skipped mirror run does not warn", async () => {
  const deps = makeDeps({ run: completed("skipped") });
  await waitForMirrorRun(deps);

  assert.match(deps.infos.join("\n"), /Mirror run was skipped/);
  assert.deepEqual(deps.warnings, []);
});

test("a failed mirror run warns and proceeds", async () => {
  const deps = makeDeps({ run: completed("failure") });
  await waitForMirrorRun(deps);

  assert.match(
    deps.warnings.join("\n"),
    /concluded 'failure'.*proceeding anyway/
  );
});

// The API reports a completed run without a conclusion while finalising it.
test("a completed run without a conclusion warns and proceeds", async () => {
  const deps = makeDeps({ run: completed(null) });
  await waitForMirrorRun(deps);

  assert.match(deps.warnings.join("\n"), /concluded 'null'.*proceeding anyway/);
});

test("a failed re-run behind a successful run warns with the re-run's URL", async () => {
  const deps = makeDeps({
    pages: [
      [
        completed("success"),
        { id: 3, status: "in_progress", conclusion: null },
      ],
      [
        completed("success"),
        { id: 3, status: "completed", conclusion: "failure" },
      ],
    ],
  });
  await waitForMirrorRun(deps);

  assert.deepEqual(
    deps.infos.filter((m) => /succeeded/.test(m)),
    []
  );
  assert.match(deps.warnings.join("\n"), /concluded 'failure'.*runs\/3\)/);
});

test("a mirror run that never concludes warns and proceeds", async () => {
  const deps = makeDeps({
    run: { id: 42, status: "in_progress", conclusion: null },
  });
  await waitForMirrorRun(deps);

  // 15 minutes of 30-second polls, plus the one that finds the deadline passed.
  assert.equal(deps.shas.length, 31);
  assert.match(
    deps.warnings.join("\n"),
    /Timed out waiting for the mirror run.*proceeding anyway/
  );
});

test("an API error warns and proceeds instead of failing the step", async () => {
  const deps = makeDeps({
    apiError: Object.assign(
      new Error("Resource not accessible by integration"),
      { status: 403 }
    ),
  });
  await waitForMirrorRun(deps);

  assert.match(
    deps.warnings.join("\n"),
    /Could not check the mirror run .*Resource not accessible.*proceeding anyway/
  );
});

test("pull_request events wait on the PR head, not the merge commit", async () => {
  const deps = makeDeps({
    run: completed("success"),
    prHeadSha: PR_HEAD_SHA,
  });
  await waitForMirrorRun(deps);

  assert.deepEqual(deps.shas, [PR_HEAD_SHA]);
});
