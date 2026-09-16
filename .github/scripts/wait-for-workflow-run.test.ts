// Unit tests for wait-for-workflow-run.ts.
//
// Run with Node's built-in test runner (no extra dependencies):
//   node --test .github/scripts/wait-for-workflow-run.test.ts

import assert from "node:assert/strict";
import test from "node:test";

import type { WorkflowRun } from "./github-script.ts";
import { waitForWorkflowRun, type GitHub } from "./wait-for-workflow-run.ts";

const OWNER = "NomicFoundation";
const REPO = "edr";
const SHA = "a2f7cf2131d0bd58d29e8a0d84a1ae61dfc7f8e1";
const TIMEOUT_MS = 15 * 60 * 1000;
const POLL_INTERVAL_MS = 30 * 1000;

// One entry per `listWorkflowRuns` call, in order; the last entry repeats for
// any further call. `undefined` is an empty run list.
function makeDeps(responses: Array<WorkflowRun | undefined>) {
  const calls: Array<Record<string, unknown>> = [];
  const infos: string[] = [];
  // Fake clock, advanced only by the fake `sleep`, so the timeout path costs
  // no wall-clock time.
  let clock = 0;

  const github: GitHub = {
    rest: {
      actions: {
        listWorkflowRuns: async (params) => {
          calls.push(params);
          const run =
            responses[Math.min(calls.length - 1, responses.length - 1)];
          return { data: { workflow_runs: run === undefined ? [] : [run] } };
        },
      },
    },
  };

  return {
    github,
    core: {
      info: (m: string) => infos.push(m),
      notice: () => {},
      warning: () => {},
    },
    owner: OWNER,
    repo: REPO,
    headSha: SHA,
    timeoutMs: TIMEOUT_MS,
    pollIntervalMs: POLL_INTERVAL_MS,
    sleep: async (ms: number) => {
      clock += ms;
    },
    now: () => clock,
    calls,
    infos,
  };
}

const running: WorkflowRun = { id: 7, status: "in_progress", conclusion: null };
const done: WorkflowRun = { id: 7, status: "completed", conclusion: "success" };

test("a concluded run is returned without sleeping", async () => {
  const deps = makeDeps([done]);
  const result = await waitForWorkflowRun({
    ...deps,
    workflowId: "edr-ci.yml",
    onMissing: "wait",
  });

  assert.deepEqual(result, { outcome: "concluded", run: done });
  assert.equal(deps.calls.length, 1);
  assert.deepEqual(deps.calls[0], {
    owner: OWNER,
    repo: REPO,
    workflow_id: "edr-ci.yml",
    head_sha: SHA,
    per_page: 1,
  });
});

test("polls until the run concludes", async () => {
  const deps = makeDeps([running, running, done]);
  const result = await waitForWorkflowRun({
    ...deps,
    workflowId: "edr-ci.yml",
    onMissing: "wait",
  });

  assert.deepEqual(result, { outcome: "concluded", run: done });
  assert.equal(deps.calls.length, 3);
  assert.equal(deps.infos.length, 2);
  assert.match(deps.infos[0] ?? "", /edr-ci\.yml .* has not concluded/);
  assert.match(deps.infos[0] ?? "", /status: in_progress/);
});

test("onMissing 'stop' returns immediately when there is no run", async () => {
  const deps = makeDeps([undefined]);
  const result = await waitForWorkflowRun({
    ...deps,
    workflowId: "mirror-docker-images.yml",
    onMissing: "stop",
  });

  assert.deepEqual(result, { outcome: "missing" });
  assert.equal(deps.calls.length, 1);
});

test("onMissing 'wait' keeps polling until the run appears", async () => {
  const deps = makeDeps([undefined, running, done]);
  const result = await waitForWorkflowRun({
    ...deps,
    workflowId: "edr-ci.yml",
    onMissing: "wait",
  });

  assert.deepEqual(result, { outcome: "concluded", run: done });
  assert.equal(deps.calls.length, 3);
  assert.match(deps.infos[0] ?? "", /status: not started/);
});

test("gives up at the deadline", async () => {
  const deps = makeDeps([running]);
  const result = await waitForWorkflowRun({
    ...deps,
    workflowId: "mirror-docker-images.yml",
    onMissing: "stop",
  });

  assert.deepEqual(result, { outcome: "timed-out" });
  // One poll per interval, plus the one that finds the deadline passed.
  assert.equal(deps.calls.length, TIMEOUT_MS / POLL_INTERVAL_MS + 1);
});

test("a run that never appears times out rather than reporting it missing", async () => {
  const deps = makeDeps([undefined]);
  const result = await waitForWorkflowRun({
    ...deps,
    workflowId: "edr-ci.yml",
    onMissing: "wait",
  });

  assert.deepEqual(result, { outcome: "timed-out" });
});
