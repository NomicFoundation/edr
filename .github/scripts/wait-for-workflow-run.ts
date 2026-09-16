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

export type WaitResult =
  | { outcome: "concluded"; run: WorkflowRun }
  | { outcome: "missing" }
  | { outcome: "timeout" };

// `onMissing` says what an empty run list means: "wait" for a run the caller
// knows was triggered and that may not be registered yet, "stop" when the run
// is optional and its absence is itself the answer.
//
// Tests pass `sleep` and `now` to fake the clock.
export async function waitForWorkflowRun({
  github,
  core,
  owner,
  repo,
  workflowId,
  headSha,
  timeoutMs,
  pollIntervalMs,
  onMissing,
  sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms)),
  now = () => Date.now(),
}: {
  github: GitHub;
  core: Core;
  owner: string;
  repo: string;
  workflowId: string;
  headSha: string;
  timeoutMs: number;
  pollIntervalMs: number;
  onMissing: "wait" | "stop";
  sleep?: (ms: number) => Promise<unknown>;
  now?: () => number;
}): Promise<WaitResult> {
  const deadline = now() + timeoutMs;

  while (true) {
    const { data } = await github.rest.actions.listWorkflowRuns({
      owner,
      repo,
      workflow_id: workflowId,
      head_sha: headSha,
      // Newest first, but a re-run keeps its original created_at, so an
      // in-flight run can sit behind a newer completed one.
      per_page: 10,
    });
    const runs = data.workflow_runs;
    const newest = runs[0];
    const pending = runs.find((run) => run.status !== "completed");

    if (newest === undefined && onMissing === "stop") {
      return { outcome: "missing" };
    }
    if (newest !== undefined && pending === undefined) {
      return { outcome: "concluded", run: newest };
    }
    if (now() >= deadline) {
      return { outcome: "timeout" };
    }

    core.info(
      `${workflowId} for ${headSha.slice(0, 12)} has not concluded ` +
        `(status: ${pending?.status ?? "not started"}); re-checking in ` +
        `${pollIntervalMs / 1000}s`
    );
    await sleep(pollIntervalMs);
  }
}
