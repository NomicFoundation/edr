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
const WORKFLOW = "edr-ci.yml";
const TIMEOUT_MS = 90_000;
const POLL_INTERVAL_MS = 30_000;

// One entry per `listWorkflowRuns` call, in order; the last entry repeats for
// any further call. A single run stands for a one-run page, `undefined` for
// an empty one.
function makeDeps(
  responses: Array<WorkflowRun | WorkflowRun[] | undefined>,
  { timeoutMs = TIMEOUT_MS }: { timeoutMs?: number } = {}
) {
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
          const response =
            responses[Math.min(calls.length - 1, responses.length - 1)];
          const workflow_runs =
            response === undefined
              ? []
              : Array.isArray(response)
                ? response
                : [response];
          return { data: { workflow_runs } };
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
    workflowId: WORKFLOW,
    headSha: SHA,
    timeoutMs,
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

test("a completed run is returned on the first poll", async () => {
  const deps = makeDeps([done]);
  const result = await waitForWorkflowRun({ ...deps, onMissing: "wait" });

  assert.deepEqual(result, { outcome: "concluded", run: done });
  assert.equal(deps.calls.length, 1);
  assert.deepEqual(deps.calls[0], {
    owner: OWNER,
    repo: REPO,
    workflow_id: WORKFLOW,
    head_sha: SHA,
    per_page: 10,
  });
});

test("polls until the run concludes", async () => {
  const deps = makeDeps([running, running, done]);
  const result = await waitForWorkflowRun({ ...deps, onMissing: "wait" });

  assert.deepEqual(result, { outcome: "concluded", run: done });
  assert.equal(deps.calls.length, 3);
  assert.equal(deps.infos.length, 2);
  assert.match(deps.infos.join("\n"), /edr-ci\.yml .* has not concluded/);
  assert.match(deps.infos.join("\n"), /status: in_progress/);
});

// A re-run of an older run keeps its created_at, so it lists behind the
// newest run even while in flight.
test("an in-flight run behind a completed one is still waited for", async () => {
  const older: WorkflowRun = { id: 3, status: "in_progress", conclusion: null };
  const olderDone: WorkflowRun = {
    id: 3,
    status: "completed",
    conclusion: "success",
  };
  const deps = makeDeps([
    [done, older],
    [done, olderDone],
  ]);
  const result = await waitForWorkflowRun({ ...deps, onMissing: "stop" });

  assert.deepEqual(result, { outcome: "concluded", run: done });
  assert.equal(deps.calls.length, 2);
});

test("onMissing 'stop' returns immediately when there is no run", async () => {
  const deps = makeDeps([undefined]);
  const result = await waitForWorkflowRun({ ...deps, onMissing: "stop" });

  assert.deepEqual(result, { outcome: "missing" });
  assert.equal(deps.calls.length, 1);
});

test("onMissing 'wait' keeps polling until the run appears", async () => {
  const deps = makeDeps([undefined, running, done]);
  const result = await waitForWorkflowRun({ ...deps, onMissing: "wait" });

  assert.deepEqual(result, { outcome: "concluded", run: done });
  assert.equal(deps.calls.length, 3);
  assert.match(deps.infos.join("\n"), /status: not started/);
});

test("gives up at the deadline", async () => {
  const deps = makeDeps([running]);
  const result = await waitForWorkflowRun({ ...deps, onMissing: "stop" });

  assert.deepEqual(result, { outcome: "timeout" });
  assert.equal(deps.calls.length, 4);
});

test("a run that concludes on the deadline poll is reported concluded", async () => {
  const deps = makeDeps([running, running, done], {
    timeoutMs: 2 * POLL_INTERVAL_MS,
  });
  const result = await waitForWorkflowRun({ ...deps, onMissing: "wait" });

  assert.deepEqual(result, { outcome: "concluded", run: done });
});

test("a run that never appears times out rather than reporting it missing", async () => {
  const deps = makeDeps([undefined]);
  const result = await waitForWorkflowRun({ ...deps, onMissing: "wait" });

  assert.deepEqual(result, { outcome: "timeout" });
});
