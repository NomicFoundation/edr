# Hardhat regression benchmark

The `Hardhat 3 Regression Benchmark` workflow (`.github/workflows/hh3-regression-benchmark.yml`) runs Hardhat's E2E regression scenarios against the EDR build under test, so EDR PRs and `main` catch EDR-side performance regressions. Hardhat's own regression benchmark cannot: it uses whatever EDR is pinned on npm.

The benchmark runs on Hardhat's self-hosted runner so measurements are comparable to Hardhat's own baselines. The job has a three-hour timeout.

The benchmark tool itself lives in Hardhat (`scripts/benchmark/regression.ts`), so what gets measured is defined there, not in EDR.

## What runs

Each scenario under Hardhat's `end-to-end/` is a real project pinned to a commit, with a `scenario.json` that declares its benchmarks under `benchmark.commands`: either a single command timed with hyperfine, or a sequence of steps timed in-process, each with a fixed run count. Entries can declare `dependsOn` prerequisites (e.g. `test solidity` runs with `--no-compile` and needs `cold compile` first). A scenario opts out with `benchmark.skip`; one whose definition is invalid (e.g. missing its commands) fails pre-flight before anything runs.

Every measured entry is reported as `<scenario> / <name>` with its wall-clock time, plus sibling `(cpu)` and `(peak RSS)` entries. When benchmarks are filtered, the selected entries' prerequisites still run, as few times as possible, but are not reported. A failure in one scenario is logged with a reproduction command and the run continues with the next; the report is still written and the exit code is non-zero.

## How the local EDR reaches the scenarios

Scenarios only see EDR through the `hardhat` package, so the workflow has to get a locally published Hardhat to depend on a locally published EDR:

1. EDR is built and published to Verdaccio as `<version>-local.<sha>` via `scripts/publish_to_verdaccio.sh`, with only the runner's platform package wired in. The sha keeps the benchmarked commit traceable in the version.
2. Hardhat's `@nomicfoundation/edr` dependency is repointed at that version, and Hardhat's own version is bumped one patch above the newer of the checkout and the latest npm release. A release version is required because plugins' peer ranges exclude prereleases; it must exceed the npm release so `--use-local` republishes Hardhat instead of skipping it, which would leave scenarios on the npm Hardhat and its npm EDR.
3. Hardhat is reinstalled and built against the local EDR, and `bench:regression --use-local --force-publish` reuses the running Verdaccio to republish Hardhat and pin every scenario to it.

A final step reads the EDR version each scenario's Hardhat actually resolves and fails the run unless all of them match the local build.

## Triggers

- **Push to `main`** records a baseline.
- **`workflow_dispatch`** runs the current commit against the Hardhat ref given in the `hardhat-ref` input. Empty means `main`, or the compat pin if one is active (see below).
- **`/bench` comment** on a PR runs the PR head. Optional `key=value` arguments follow the command: `hardhat-ref=<branch|sha>`, `scenarios=<globs>` and `benchmarks=<globs>`; quote values containing spaces, e.g. `benchmarks="cold compile"`.

`/bench` is restricted. The commenter must be an owner, member or collaborator, and the PR must come from a branch in this repository: the self-hosted runner must not execute code from forks. The workflow reacts to the comment with 👀, waits up to 30 minutes for the EDR CI run of the PR head to pass, then posts a status comment: either that the benchmark started, or why it did not (CI not green, fork, unresolvable pin). Commenting `/bench` again on the same PR supersedes an in-progress run.

Trigger resolution and gating are implemented in `.github/scripts/resolve-regression-trigger.ts`, covered by unit tests that run in EDR CI as `pnpm test:workflows`. Note that GitHub runs `issue_comment` workflows from the default branch, and push only fires for `main`, so a PR that changes this workflow cannot exercise the change with `/bench`. Use `workflow_dispatch` on the PR branch instead, which works because the workflow already exists on `main`.

## Selecting what runs

Both filters are comma-separated globs, forwarded to `bench:regression` as `--scenarios` and `--benchmarks`.

- **Scenarios** select projects by directory name (e.g. `1inch*`). Default `*`, all projects.
- **Benchmarks** select by command or step name within each project, the part after `<project> /` in a report (e.g. `test solidity`, `cold compile`, `*compile*`). Default `test solidity*,test mocha*,test vitest*`: test execution including verbosity variants, since EDR affects test execution rather than compilation. Pass `*` for the full suite.

The defaults apply to every trigger, including `main` baselines.

## Results

Every run is compared against the last data point recorded in the `hardhat3` dataset of the [results repository](https://github.com/nomic-foundation-automation/edr-benchmark-results), i.e. the previous push to `main`; a benchmark that is more than 10% slower counts as a regression. The measurements from `main` are visualized [here](https://nomic-foundation-automation.github.io/edr-benchmark-results/hardhat3/).

- **`/bench` and dispatch runs** fail on a regression. `/bench` runs also get a ✅/❌ result comment on the PR; a ❌ is either a regression or an infrastructure failure, and the run's step annotations say which.
- **Baseline runs** never fail on a regression. The data point is recorded regardless so the baseline tracks the newest commit, and a regression is reported to Slack instead. Since the comparison is against the previous data point, a regression alerts once and then becomes the new normal. A baseline run that fails for any other reason also notifies Slack.

## Hardhat compat pin

When an EDR change breaks compatibility with current Hardhat `main`, the default target fails until the matching Hardhat PR merges. The compat pin lets an EDR branch benchmark against a commit on that open Hardhat PR instead. Check in `.github/hardhat-compat-pin.json`:

```json
{ "pr": <Hardhat PR number>, "sha": "<full 40-hex commit sha>", "reason": "..." }
```

`pr` is a PR in `NomicFoundation/hardhat` — not a fork, so the runner can check the sha out directly. `sha` is a full 40-character commit sha on that PR; the pin follows the sha, not the branch, so update it if the Hardhat PR gains commits you need. `reason` is an optional free-form note.

### How the pin is resolved

Every run that would otherwise default to Hardhat `main` — push baselines, and dispatch / `/bench` runs without an explicit `hardhat-ref` — reads the pin at the EDR commit being benchmarked and acts on the state of the Hardhat PR:

- **Open**: benchmarks against the pinned sha. The `/bench` status comment notes the active pin.
- **Merged**: reverts to `main` and emits a notice that the pin file can be removed.
- **Closed without merging**: reverts to `main` with a warning.
- **Malformed** (invalid JSON, missing or invalid `pr` or `sha`): fails instead of silently benchmarking `main`, which is exactly the incompatible state the pin exists to avoid. On `/bench` the error is posted back to the PR and the run is skipped; on push and dispatch the setup job fails.

An explicit `hardhat-ref` (the dispatch input or the `hardhat-ref=` comment argument) always wins over the pin.

### Validation

PRs that touch the pin file run `.github/workflows/validate-hardhat-compat-pin.yml`. It parses the file with the same rules the benchmark uses, checks that the Hardhat PR exists and, for an open PR, that the pinned sha is reachable from the PR's head. This catches a broken pin at review time: the benchmark itself may never run on the PR that adds the pin. A merged or closed Hardhat PR is reported as a notice or warning rather than a failure, so the check doesn't break retroactively when the Hardhat PR's state changes; a missing file passes, since deleting the pin is the normal cleanup.

### Lifecycle

1. On the EDR PR, add the pin pointing at the Hardhat PR.
2. Merge the EDR PR. Baselines on `main` keep using the pin while the Hardhat PR is open.
3. Once the Hardhat PR merges, runs revert to `main` automatically. Remove the pin file in a follow-up PR.
