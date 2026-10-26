// Validate the Hardhat compat pin (`.github/hardhat-compat-pin.json`) in CI.
//
// Run by validate-hardhat-compat-pin.yml on PRs that touch the pin file, so a
// broken pin is caught at review time instead of in the next regression
// benchmark run — which may never happen on the PR that adds the pin (e.g. if
// `/bench` was not commented there).
//
// Checks:
//   - the file parses, matches the expected shape and names an existing
//     Hardhat PR, as decided by the same resolver the workflows use;
//   - for an open Hardhat PR, the pinned sha is a commit reachable from the
//     PR's head (i.e. actually part of what would be benchmarked).
//
// Non-failures:
//   - a missing file: deleting the pin is the normal cleanup after the
//     Hardhat PR merges;
//   - a merged/closed Hardhat PR: the resolver reverts to `main` in that
//     state, so the pin is inert, not broken. The resolver reports it as a
//     notice or warning. This keeps the check green when the Hardhat PR
//     state changes after the pin lands.
//
// See README.md for the conventions these scripts follow.

import { errorStatus, type Core, type PullRequest } from "./github-script.ts";
import {
  HARDHAT_OWNER,
  HARDHAT_PIN_PATH,
  HARDHAT_REPO,
  pinnedPrName,
  readHardhatPin,
  resolveHardhatRef,
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
  const raw = readHardhatPin(pinPath);
  if (raw === undefined) {
    core.info(`${HARDHAT_PIN_PATH} not present; nothing to validate.`);
    return;
  }

  // A merged or closed Hardhat PR is reported by the resolver and leaves the
  // pin unset.
  const { pin, hardhatPr } = await resolveHardhatRef({ github, core, raw });
  if (pin === undefined) {
    return;
  }
  const prName = pinnedPrName(pin);

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
