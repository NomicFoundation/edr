// Wait for the workflow run recorded against a head SHA to conclude.
//
// See README.md for the conventions these scripts follow.

import type { Core, WorkflowRun } from "./github-script.ts";

export interface GitHub {
  rest: {
    actions: {
      listWorkflowRuns: (params: {
        owner: string;
        repo: string;
        workflow_id: string;
        head_sha: string;
        per_page: number;
      }) => Promise<{ data: { workflow_runs: WorkflowRun[] } }>;
    };
  };
}

export interface Clock {
  sleep: (ms: number) => Promise<unknown>;
  now: () => number;
}

const REAL_CLOCK: Clock = {
  sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  now: () => Date.now(),
};

export const POLL_INTERVAL_MS = 30 * 1000; // 30 seconds
export const RUNS_PER_PAGE = 10;

export type WaitResult =
  | { outcome: "concluded"; run: WorkflowRun }
  | { outcome: "missing" }
  | { outcome: "timeout" };

// `onMissing` says what an empty run list means: "wait" for a run the caller
// knows was triggered and that may not be registered yet, "stop" when the run
// is optional and its absence is itself the answer.
//
// `waitFor` says which runs must complete. "newest" waits for the newest run
// only. "any" waits until no run on the page is in flight and reports the one
// it waited on last, for callers where an older re-run still does the work.
export async function waitForWorkflowRun({
  github,
  core,
  owner,
  repo,
  workflowId,
  headSha,
  timeoutMs,
  onMissing,
  waitFor,
  clock = REAL_CLOCK,
}: {
  github: GitHub;
  core: Core;
  owner: string;
  repo: string;
  workflowId: string;
  headSha: string;
  timeoutMs: number;
  onMissing: "wait" | "stop";
  waitFor: "newest" | "any";
  clock?: Clock;
}): Promise<WaitResult> {
  const deadline = clock.now() + timeoutMs;
  let waitedOn: number | undefined;

  while (true) {
    const { data } = await github.rest.actions.listWorkflowRuns({
      owner,
      repo,
      workflow_id: workflowId,
      head_sha: headSha,
      // Newest first, but a re-run keeps its original created_at, so an
      // in-flight run can sit behind a newer completed one.
      per_page: RUNS_PER_PAGE,
    });
    const runs = data.workflow_runs;
    const newest = runs[0];
    const pending = (waitFor === "newest" ? runs.slice(0, 1) : runs).find(
      (run) => run.status !== "completed"
    );

    if (newest === undefined && onMissing === "stop") {
      return { outcome: "missing" };
    }
    if (newest !== undefined && pending === undefined) {
      const run = runs.find((run) => run.id === waitedOn) ?? newest;
      return { outcome: "concluded", run };
    }
    waitedOn = pending?.id;
    if (clock.now() >= deadline) {
      return { outcome: "timeout" };
    }

    core.info(
      `${workflowId} for ${headSha.slice(0, 12)} has not concluded ` +
        `(status: ${pending?.status ?? "not started"}); re-checking in ` +
        `${POLL_INTERVAL_MS / 1000}s`
    );
    await clock.sleep(POLL_INTERVAL_MS);
  }
}
