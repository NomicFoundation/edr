// Resolve refs, authorize, and gate the EDR regression benchmark trigger.
//
// By event:
//   push                -> baseline run of HEAD against Hardhat main
//   workflow_dispatch   -> run HEAD against the requested Hardhat ref
//   issue_comment        -> a `/bench` comment on a same-repo PR, gated on the
//                          commenter's permissions and EDR CI being green
//
// Whenever a run would default to Hardhat `main` (push baselines, and
// dispatch/`/bench` runs without an explicit hardhat-ref), the resolver honors
// an optional "compat pin" (HARDHAT_PIN_PATH below): while the pinned Hardhat
// PR is open, the benchmark runs against the pinned commit instead of `main`;
// once that PR is merged (or closed), runs revert to `main` automatically.

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

// Optional Hardhat compatibility pin (see hardhat-compat-pin.cjs for the file
// format). While the pinned Hardhat PR is open, runs that don't name an
// explicit hardhat-ref benchmark against the pinned sha; once it's merged (or
// closed) they revert to `main` automatically. Delete the file after the
// Hardhat PR merges.
const {
  HARDHAT_OWNER,
  HARDHAT_REPO,
  HARDHAT_PIN_PATH,
  parseHardhatPin,
} = require("./hardhat-compat-pin.cjs");

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

  // Resolve the Hardhat ref for runs that didn't name one explicitly: `main`,
  // unless a compat pin (HARDHAT_PIN_PATH) exists on `ref` and its Hardhat PR
  // is still open. Returns { ref, pin? }, with `pin` set only when the pinned
  // sha is actually used. Throws on a malformed pin (or an unreadable pinned
  // PR) so misconfiguration fails loudly instead of silently benchmarking
  // `main` — which is exactly the incompatible state the pin exists to avoid.
  async function resolveDefaultHardhatRef(ref) {
    let raw;
    try {
      const { data } = await github.rest.repos.getContent({
        owner,
        repo,
        path: HARDHAT_PIN_PATH,
        ref,
      });
      raw = Buffer.from(data.content, "base64").toString("utf8");
    } catch (e) {
      if (e.status === 404) return { ref: "main" }; // no pin
      throw e;
    }

    const pin = parseHardhatPin(raw);

    const { data: hardhatPr } = await github.rest.pulls.get({
      owner: HARDHAT_OWNER,
      repo: HARDHAT_REPO,
      pull_number: pin.pr,
    });
    const prName = `${HARDHAT_OWNER}/${HARDHAT_REPO}#${pin.pr}`;
    if (hardhatPr.merged) {
      core.notice(
        `Hardhat compat pin ${prName} has been merged; benchmarking against ` +
          `Hardhat main. ${HARDHAT_PIN_PATH} can now be removed.`
      );
      return { ref: "main" };
    }
    if (hardhatPr.state === "closed") {
      core.warning(
        `Hardhat compat pin ${prName} was closed without merging; ` +
          `benchmarking against Hardhat main. Remove or update ${HARDHAT_PIN_PATH}.`
      );
      return { ref: "main" };
    }
    core.info(`Hardhat compat pin active: ${prName} @ ${pin.sha}`);
    return { ref: pin.sha, pin };
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
    hardhatRef = (await resolveDefaultHardhatRef(edrRef)).ref;
    isBaseline = true;
  } else if (eventName === "workflow_dispatch") {
    shouldRun = true;
    edrRef = context.sha;
    // An explicit hardhat-ref input always wins over the compat pin.
    hardhatRef =
      context.payload.inputs["hardhat-ref"] ||
      (await resolveDefaultHardhatRef(edrRef)).ref;
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

        hardhatRef = parseParam("hardhat-ref");
        scenarioFilter = parseParam("scenarios") || DEFAULT_SCENARIO_FILTER;
        benchmarkFilter = parseParam("benchmarks") || DEFAULT_BENCHMARK_FILTER;

        // No explicit hardhat-ref= → Hardhat main, or the compat pin if one
        // is active on the PR head. A malformed pin is reported back to the
        // PR (instead of failing the job with no feedback) and skips the run.
        let pinNote = "";
        let pinError;
        if (hardhatRef === "") {
          try {
            const resolved = await resolveDefaultHardhatRef(edrRef);
            hardhatRef = resolved.ref;
            if (resolved.pin !== undefined) {
              pinNote =
                ` (compat pin for ${HARDHAT_OWNER}/${HARDHAT_REPO}` +
                `#${resolved.pin.pr})`;
            }
          } catch (e) {
            hardhatRef = "main";
            pinError = e instanceof Error ? e.message : String(e);
          }
        }

        // Gate on EDR CI being green for the PR head before spending
        // ~3h on the self-hosted runner.
        const green =
          pinError === undefined && (await waitForEdrCi(pr.head.sha));
        if (pinError !== undefined) {
          await postComment(
            `⚠️ Could not resolve the Hardhat compat pin, so the regression ` +
              `benchmark was not started: ${pinError}`
          );
        } else if (green) {
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
              `\`${edrRef.slice(0, 12)}\` against Hardhat ` +
              `\`${hardhatRef}\`${pinNote}${filterNote}.`
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
