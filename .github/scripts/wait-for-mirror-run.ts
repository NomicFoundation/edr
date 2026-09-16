// Wait for the GHCR mirror run for this commit before pulling node images.
//
// On PRs that touch mirror-docker-images.yml, its run and the docker jobs in
// edr-npm-release.yml start in parallel, so a pull can race ahead of the tag
// the mirror is still copying and fail with "manifest unknown".
//
// Best-effort by design — it never fails the job. Most commits have no mirror
// run at all, a fork PR's mirror job skips itself, and a failed weekly
// re-sync shares main's head SHA; in each of those the tags a pull needs are
// already in place. The pull itself is the loud, authoritative failure for a
// tag that genuinely isn't there.
//
// Loaded by actions/github-script in .github/actions/select-node-image.
// See README.md for the conventions these scripts follow.

import type { Core } from "./github-script.ts";
import { waitForWorkflowRun, type GitHub } from "./wait-for-workflow-run.ts";

const MIRROR_WORKFLOW = "mirror-docker-images.yml";

// How long to wait for the mirror run before giving up, and how often to
// re-check while waiting. Copying the node tags takes a couple of minutes;
// the margin is for a queued run. Overshooting costs a wait, not a failure.
const TIMEOUT_MS = 15 * 60 * 1000; // 15 minutes
const POLL_INTERVAL_MS = 30 * 1000; // 30 seconds

export interface Context {
  repo: { owner: string; repo: string };
  sha: string;
  serverUrl: string;
  payload: { pull_request?: { head: { sha: string } } };
}

// `sleep` and `now` are seams for testing; see wait-for-workflow-run.ts.
export async function waitForMirrorRun({
  github,
  context,
  core,
  sleep,
  now,
}: {
  github: GitHub;
  context: Context;
  core: Core;
  sleep?: (ms: number) => Promise<unknown>;
  now?: () => number;
}): Promise<void> {
  const { owner, repo } = context.repo;
  // For pull_request events the mirror run is recorded against the PR head
  // SHA, not the merge commit `context.sha` points at.
  const headSha = context.payload.pull_request?.head.sha ?? context.sha;

  const result = await waitForWorkflowRun({
    github,
    core,
    owner,
    repo,
    workflowId: MIRROR_WORKFLOW,
    headSha,
    timeoutMs: TIMEOUT_MS,
    pollIntervalMs: POLL_INTERVAL_MS,
    onMissing: "stop",
    sleep,
    now,
  });

  if (result.outcome === "missing") {
    core.info(`No mirror run for ${headSha}; not waiting.`);
    return;
  }
  if (result.outcome === "timed-out") {
    core.warning(
      `Timed out waiting for the mirror run for ${headSha}; proceeding anyway`
    );
    return;
  }

  const { run } = result;
  const url = `${context.serverUrl}/${owner}/${repo}/actions/runs/${run.id}`;
  switch (run.conclusion) {
    case "success":
      core.info(`Mirror run succeeded: ${url}`);
      break;
    // The mirror job skips itself on fork PRs; the tags it would have
    // re-copied already exist.
    case "skipped":
      core.info(`Mirror run was skipped: ${url}`);
      break;
    default:
      core.warning(
        `Mirror run for ${headSha} concluded '${run.conclusion}' (${url}); ` +
          `proceeding anyway`
      );
  }
}
