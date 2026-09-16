// Wait for the workflow run recorded against a head SHA to conclude.
//
// Shared by the two places that have to order themselves against another run
// for the same commit: hh3-regression-benchmark waits for edr-ci.yml, and the
// select-node-image action waits for mirror-docker-images.yml (see
// wait-for-mirror-run.ts). They differ only in what an absent run means,
// hence `onMissing`.
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
  | { outcome: "missing" } // only with `onMissing: "stop"`
  | { outcome: "timed-out" };

// `onMissing` says what an empty run list means: "wait" for a run the caller
// knows was triggered and that may not be registered yet, "stop" when the run
// is optional and its absence is itself the answer.
//
// `sleep` and `now` are seams for testing the polling and timeout paths.
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
      per_page: 1,
    });
    const run = data.workflow_runs[0];

    if (run === undefined && onMissing === "stop") {
      return { outcome: "missing" };
    }
    if (run !== undefined && run.status === "completed") {
      return { outcome: "concluded", run };
    }
    if (now() >= deadline) {
      return { outcome: "timed-out" };
    }

    core.info(
      `${workflowId} for ${headSha.slice(0, 12)} has not concluded ` +
        `(status: ${run?.status ?? "not started"}); re-checking in ` +
        `${pollIntervalMs / 1000}s`
    );
    await sleep(pollIntervalMs);
  }
}
