// Validate the Hardhat compat pin (`.github/hardhat-compat-pin.json`) in CI.
//
// Run by validate-hardhat-compat-pin.yml on PRs that touch the pin file, so a
// broken pin is caught at review time instead of in the next regression
// benchmark run — which may never happen on the PR that adds the pin (e.g. if
// `/bench` was not commented there).
//
// Checks:
//   - the file parses and matches the expected shape, using the same parser
//     the benchmark's trigger resolver uses;
//   - the referenced Hardhat PR exists in NomicFoundation/hardhat;
//   - for an open Hardhat PR, the pinned sha is a commit reachable from the
//     PR's head (i.e. actually part of what would be benchmarked).
//
// Non-failures:
//   - a missing file: deleting the pin is the normal cleanup after the
//     Hardhat PR merges;
//   - a merged/closed Hardhat PR: the resolver reverts to `main` in that
//     state, so the pin is inert, not broken — reported as a notice/warning
//     (also keeps this check from breaking retroactively when the Hardhat PR
//     state changes after the pin was added).
//
// See README.md for the conventions these scripts follow.

import { readFileSync } from "node:fs";

import {
  errorCode,
  errorStatus,
  type Core,
  type PullRequest,
} from "./github-script.ts";
import {
  HARDHAT_OWNER,
  HARDHAT_PIN_PATH,
  HARDHAT_REPO,
  parseHardhatPin,
} from "./hardhat-compat-pin.ts";

interface GitHub {
  rest: {
    pulls: {
      get: (params: {
        owner: string;
        repo: string;
        pull_number: number;
      }) => Promise<{ data: PullRequest }>;
    };
    repos: {
      compareCommitsWithBasehead: (params: {
        owner: string;
        repo: string;
        basehead: string;
      }) => Promise<{ data: { status: string } }>;
    };
  };
}

// `pinPath` is overridable for tests; in CI the file is read from the
// checked-out workspace.
export async function validateHardhatCompatPin({
  github,
  core,
  pinPath = HARDHAT_PIN_PATH,
}: {
  github: GitHub;
  core: Core;
  pinPath?: string;
}): Promise<void> {
  let raw: string;
  try {
    raw = readFileSync(pinPath, "utf8");
  } catch (e) {
    if (errorCode(e) === "ENOENT") {
      core.info(`${HARDHAT_PIN_PATH} not present; nothing to validate.`);
      return;
    }
    throw e;
  }

  // Throws a descriptive error on invalid JSON or a malformed shape.
  const pin = parseHardhatPin(raw);

  const prName = `${HARDHAT_OWNER}/${HARDHAT_REPO}#${pin.pr}`;
  let hardhatPr: PullRequest;
  try {
    ({ data: hardhatPr } = await github.rest.pulls.get({
      owner: HARDHAT_OWNER,
      repo: HARDHAT_REPO,
      pull_number: pin.pr,
    }));
  } catch (e) {
    if (errorStatus(e) === 404) {
      throw new Error(`${HARDHAT_PIN_PATH}: Hardhat PR ${prName} not found`);
    }
    throw e;
  }

  if (hardhatPr.merged) {
    core.notice(
      `Hardhat compat pin ${prName} has already been merged; the benchmark ` +
        `will run against Hardhat main. ${HARDHAT_PIN_PATH} can be removed.`
    );
    return;
  }
  if (hardhatPr.state === "closed") {
    core.warning(
      `Hardhat compat pin ${prName} was closed without merging; the ` +
        `benchmark will run against Hardhat main. Remove or update ` +
        `${HARDHAT_PIN_PATH}.`
    );
    return;
  }

  // The pin must reference a commit that's part of the open PR: reachable
  // from its head. (Commits already on main are also "reachable", but such a
  // pin is merely redundant, not broken.)
  let comparison: { status: string };
  try {
    ({ data: comparison } = await github.rest.repos.compareCommitsWithBasehead({
      owner: HARDHAT_OWNER,
      repo: HARDHAT_REPO,
      basehead: `${pin.sha}...${hardhatPr.head.sha}`,
    }));
  } catch (e) {
    if (errorStatus(e) === 404) {
      throw new Error(
        `${HARDHAT_PIN_PATH}: pinned sha ${pin.sha} does not exist in ` +
          `${HARDHAT_OWNER}/${HARDHAT_REPO}`
      );
    }
    throw e;
  }
  if (comparison.status !== "identical" && comparison.status !== "ahead") {
    throw new Error(
      `${HARDHAT_PIN_PATH}: pinned sha ${pin.sha} is not reachable from the ` +
        `head of ${prName} (${hardhatPr.head.sha}); pin a commit on that PR ` +
        `(comparison status: ${comparison.status})`
    );
  }

  core.info(
    `Hardhat compat pin OK: ${prName} (open) @ ${pin.sha}` +
      (pin.reason ? ` — ${pin.reason}` : "")
  );
}
