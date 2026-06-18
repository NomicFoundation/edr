// Resolve refs, authorize, and gate the EDR regression benchmark trigger.
//
// Invoked from .github/workflows/edr-regression-benchmark.yml via
// actions/github-script, which provides the `github`, `context` and `core`
// objects. Sets the job outputs `should_run`, `edr_ref`, `hardhat_ref` and
// `is_baseline`.
//
// By event:
//   push                -> baseline run of HEAD against Hardhat main
//   workflow_dispatch   -> run HEAD against the requested Hardhat ref
//   issue_comment        -> a `/bench` comment on a same-repo PR, gated on the
//                          commenter's permissions and EDR CI being green

// How long to wait for the EDR CI run to conclude before giving up, and how
// often to re-check while waiting. Tunable independently.
const CI_WAIT_TIMEOUT_MS = 30 * 60 * 1000; // 30 minutes
const CI_POLL_INTERVAL_MS = 30 * 1000; // 30 seconds

module.exports = async ({ github, context, core }) => {
  const { owner, repo } = context.repo;
  const fullName = `${owner}/${repo}`;
  const eventName = context.eventName;

  let shouldRun = false;
  let edrRef = "";
  let hardhatRef = "main";
  let isBaseline = false;

  // Wait for the EDR CI workflow run for `sha` to conclude. Returns true only
  // if it completed successfully. Polls until CI_WAIT_TIMEOUT_MS elapses.
  async function waitForEdrCi(sha) {
    const deadline = Date.now() + CI_WAIT_TIMEOUT_MS;
    while (Date.now() < deadline) {
      const { data } = await github.rest.actions.listWorkflowRuns({
        owner,
        repo,
        workflow_id: "edr-ci.yml",
        head_sha: sha,
        per_page: 1,
      });
      const run = data.workflow_runs[0];
      if (run !== undefined && run.status === "completed") {
        core.info(`EDR CI run ${run.id} concluded: ${run.conclusion}`);
        return run.conclusion === "success";
      }
      core.info(
        `EDR CI for ${sha.slice(0, 12)} not finished yet ` +
          `(status: ${run?.status ?? "not started"}); waiting...`,
      );
      await new Promise((r) => setTimeout(r, CI_POLL_INTERVAL_MS));
    }
    core.warning("Timed out waiting for EDR CI to conclude");
    return false;
  }

  async function postComment(body) {
    if (eventName !== "issue_comment") return;
    await github.rest.issues.createComment({
      owner,
      repo,
      issue_number: context.payload.issue.number,
      body,
    });
  }

  if (eventName === "push") {
    shouldRun = true;
    edrRef = context.sha;
    hardhatRef = "main";
    isBaseline = true;
  } else if (eventName === "workflow_dispatch") {
    shouldRun = true;
    edrRef = context.sha;
    hardhatRef = context.payload.inputs["hardhat-ref"] || "main";
    isBaseline = false;
  } else if (eventName === "issue_comment") {
    const comment = context.payload.comment;
    const assoc = comment.author_association;
    const allowed = ["OWNER", "MEMBER", "COLLABORATOR"];

    // Acknowledge the request.
    try {
      await github.rest.reactions.createForIssueComment({
        owner,
        repo,
        comment_id: comment.id,
        content: "eyes",
      });
    } catch (e) {
      core.warning(`Could not add reaction: ${e.message}`);
    }

    if (!allowed.includes(assoc)) {
      core.warning(
        `Comment author ${comment.user.login} (${assoc}) is not ` +
          `authorized to trigger benchmarks.`,
      );
    } else {
      const { data: pr } = await github.rest.pulls.get({
        owner,
        repo,
        pull_number: context.payload.issue.number,
      });

      if (pr.head.repo.full_name !== fullName) {
        await postComment(
          "🚫 Regression benchmarks can only run for branches in " +
            "this repository, not forks (the self-hosted runner must " +
            "not execute untrusted code). Push your branch to " +
            `\`${fullName}\` and comment \`/bench\` again.`,
        );
      } else {
        edrRef = pr.head.sha;
        isBaseline = false;

        const match = comment.body.match(/hardhat-ref=(\S+)/);
        hardhatRef = match ? match[1] : "main";

        // Gate on EDR CI being green for the PR head before spending
        // ~3h on the self-hosted runner.
        const green = await waitForEdrCi(pr.head.sha);
        if (green) {
          shouldRun = true;
          await postComment(
            `🚀 Starting regression benchmark for \`${edrRef.slice(0, 12)}\` ` +
              `against Hardhat \`${hardhatRef}\`.`,
          );
        } else {
          await postComment(
            "⏳ EDR CI for this commit hasn't passed yet, so the " +
              "regression benchmark was not started. Comment " +
              "`/bench` again once CI is green.",
          );
        }
      }
    }
  }

  core.setOutput("should_run", String(shouldRun));
  core.setOutput("edr_ref", edrRef);
  core.setOutput("hardhat_ref", hardhatRef);
  core.setOutput("is_baseline", String(isBaseline));
  core.info(
    `should_run=${shouldRun} edr_ref=${edrRef} ` +
      `hardhat_ref=${hardhatRef} is_baseline=${isBaseline}`,
  );
};
