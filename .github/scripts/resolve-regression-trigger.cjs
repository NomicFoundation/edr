// Resolve refs, authorize, and gate the EDR regression benchmark trigger.
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

// Default filters when a run doesn't specify them. The resolver always emits a
// concrete value for both (never empty), so the workflow can pass --scenarios /
// --benchmarks unconditionally; `*` matches everything.
//
// Scenarios default to all. Benchmarks default to just the test-execution
// entries — EDR affects EVM execution, not Solidity compilation, so we skip the
// expensive compile ones to save CI time. Override per run via the workflow
// inputs / `scenarios=`/`benchmarks=` comment args; pass `*` for the full suite.
const DEFAULT_SCENARIO_FILTER = "*";
const DEFAULT_BENCHMARK_FILTER = "test solidity,test mocha,test vitest";

module.exports = async ({ github, context, core }) => {
  const { owner, repo } = context.repo;
  const fullName = `${owner}/${repo}`;
  const eventName = context.eventName;
  const runUrl = `${context.serverUrl}/${owner}/${repo}/actions/runs/${context.runId}`;

  let shouldRun = false;
  let edrRef = "";
  let hardhatRef = "main";
  let isBaseline = false;
  // Glob(s) selecting which projects / benchmarks to run (forwarded verbatim to
  // bench:regression's --scenarios / --benchmarks). Both default to their
  // DEFAULT_* for every trigger (including the main baseline) and are overridable
  // on dispatch/`/bench`.
  let scenarioFilter = DEFAULT_SCENARIO_FILTER;
  let benchmarkFilter = DEFAULT_BENCHMARK_FILTER;

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
          `(status: ${run?.status ?? "not started"}); waiting...`
      );
      await new Promise((r) => setTimeout(r, CI_POLL_INTERVAL_MS));
    }
    core.warning("Timed out waiting for EDR CI to conclude");
    return false;
  }

  // Cosmetic side effects (reactions, status comments) must never fail the job:
  // the gating decision (`should_run`) is the only thing that matters. Run them
  // through this wrapper so any API rejection — insufficient token permissions,
  // rate limits, transient 5xx — degrades to a warning instead of aborting.
  async function bestEffort(description, fn) {
    try {
      await fn();
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      core.warning(`${description} failed (ignored): ${message}`);
    }
  }

  async function postComment(body) {
    if (eventName !== "issue_comment") return;
    await bestEffort("Posting status comment", () =>
      github.rest.issues.createComment({
        owner,
        repo,
        issue_number: context.payload.issue.number,
        body,
      })
    );
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
    scenarioFilter =
      context.payload.inputs["scenario-filter"] || DEFAULT_SCENARIO_FILTER;
    benchmarkFilter =
      context.payload.inputs["benchmark-filter"] || DEFAULT_BENCHMARK_FILTER;
    isBaseline = false;
  } else if (eventName === "issue_comment") {
    const comment = context.payload.comment;
    const assoc = comment.author_association;
    const allowed = ["OWNER", "MEMBER", "COLLABORATOR"];

    // Acknowledge the request.
    await bestEffort("Adding reaction", () =>
      github.rest.reactions.createForIssueComment({
        owner,
        repo,
        comment_id: comment.id,
        content: "eyes",
      })
    );

    if (!allowed.includes(assoc)) {
      core.warning(
        `Comment author ${comment.user.login} (${assoc}) is not ` +
          `authorized to trigger benchmarks.`
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
            `\`${fullName}\` and comment \`/bench\` again.`
        );
      } else {
        edrRef = pr.head.sha;
        isBaseline = false;

        // Parse `key=value` or `key="value with spaces"` (command/step globs
        // like "cold compile" contain spaces, so quotes are supported).
        const parseParam = (key) => {
          const m = comment.body.match(
            new RegExp(`${key}=(?:"([^"]*)"|(\\S+))`)
          );
          return m ? (m[1] ?? m[2]) : "";
        };

        hardhatRef = parseParam("hardhat-ref") || "main";
        scenarioFilter = parseParam("scenarios") || DEFAULT_SCENARIO_FILTER;
        benchmarkFilter = parseParam("benchmarks") || DEFAULT_BENCHMARK_FILTER;

        // Gate on EDR CI being green for the PR head before spending
        // ~3h on the self-hosted runner.
        const green = await waitForEdrCi(pr.head.sha);
        if (green) {
          shouldRun = true;
          // Only mention a filter that actually narrows the run (`*` = all).
          const filterNotes = [
            scenarioFilter !== "*" && `projects matching \`${scenarioFilter}\``,
            benchmarkFilter !== "*" &&
              `benchmarks matching \`${benchmarkFilter}\``,
          ].filter(Boolean);
          const filterNote = filterNotes.length
            ? ` (${filterNotes.join(", ")})`
            : "";
          await postComment(
            `🚀 [Starting regression benchmark](${runUrl}) for ` +
              `\`${edrRef.slice(0, 12)}\` against Hardhat \`${hardhatRef}\`${filterNote}.`
          );
        } else {
          await postComment(
            "⏳ EDR CI for this commit hasn't passed yet, so the " +
              "regression benchmark was not started. Comment " +
              "`/bench` again once CI is green."
          );
        }
      }
    }
  }

  core.setOutput("should_run", String(shouldRun));
  core.setOutput("edr_ref", edrRef);
  core.setOutput("hardhat_ref", hardhatRef);
  core.setOutput("is_baseline", String(isBaseline));
  core.setOutput("scenario_filter", scenarioFilter);
  core.setOutput("benchmark_filter", benchmarkFilter);
  core.info(
    `should_run=${shouldRun} edr_ref=${edrRef} ` +
      `hardhat_ref=${hardhatRef} is_baseline=${isBaseline} ` +
      `scenario_filter=${scenarioFilter} benchmark_filter=${benchmarkFilter}`
  );
};
