// The Hardhat compatibility pin, with helpers to read it and to resolve the
// Hardhat ref it selects.
//
// The pin is an optional file checked into the EDR repo while a breaking EDR
// change needs a not-yet-merged Hardhat counterpart. Format:
//
//   {
//     "pr": <Hardhat PR number>,
//     "sha": "<full 40-hex commit sha on that PR>",
//     "reason": "<optional free-form note>"
//   }
//
// See README.md for the conventions these scripts follow.

import { readFileSync } from "node:fs";

import {
  errorCode,
  errorStatus,
  type Core,
  type PullRequest,
} from "./github-script.ts";

// The Hardhat repository the pin refers to. The pinned PR must live here
// rather than in a fork, so the benchmark job can check the sha out directly.
export const HARDHAT_OWNER = "NomicFoundation";
export const HARDHAT_REPO = "hardhat";

// Where the pin lives in the EDR repo.
export const HARDHAT_PIN_PATH = ".github/hardhat-compat-pin.json";

// The ref a run uses when no pin applies.
const DEFAULT_HARDHAT_REF = "main";

export interface HardhatPin {
  pr: number;
  sha: string;
  // Free-form and never validated, so it is whatever the file contained.
  reason?: unknown;
}

// The resolver reads only the pinned Hardhat PR's state from `github`.
interface GitHub {
  rest: {
    pulls: {
      get: (params: {
        owner: string;
        repo: string;
        pull_number: number;
      }) => Promise<{ data: PullRequest }>;
    };
  };
}

function isHardhatPin(value: unknown): value is HardhatPin {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const { pr, sha } = value as { pr?: unknown; sha?: unknown };

  return (
    typeof pr === "number" &&
    Number.isInteger(pr) &&
    pr > 0 &&
    typeof sha === "string" &&
    /^[0-9a-f]{40}$/i.test(sha)
  );
}

// The pinned PR as `owner/repo#number`, for log and error messages.
export function pinnedPrName(pin: HardhatPin): string {
  return `${HARDHAT_OWNER}/${HARDHAT_REPO}#${pin.pr}`;
}

// Parse and validate the raw file contents. Returns the pin with a lowercased
// sha; throws a descriptive error on invalid JSON or a malformed shape.
export function parseHardhatPin(raw: string): HardhatPin {
  let pin: unknown;
  try {
    pin = JSON.parse(raw);
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    throw new Error(`${HARDHAT_PIN_PATH} is not valid JSON: ${message}`);
  }

  if (!isHardhatPin(pin)) {
    throw new Error(
      `${HARDHAT_PIN_PATH} must contain {"pr": <Hardhat PR number>, ` +
        `"sha": "<full 40-hex commit sha>"}; got: ${raw.trim()}`
    );
  }

  return { ...pin, sha: pin.sha.toLowerCase() };
}

// Read the raw pin file from a checkout. `undefined` means there is no pin.
// Any other read error propagates, because an unreadable pin is not a missing
// one and treating it as such would silently use `main`. `pinPath` is
// overridable for tests.
export function readHardhatPin(
  pinPath: string = HARDHAT_PIN_PATH
): string | undefined {
  try {
    return readFileSync(pinPath, "utf8");
  } catch (e) {
    if (errorCode(e) === "ENOENT") {
      return undefined;
    }
    throw e;
  }
}

// The outcome of resolving the pin. `pin` and `hardhatPr` are set together,
// and only when the pinned sha is used.
export type ResolvedHardhatRef =
  | { ref: string; pin?: undefined; hardhatPr?: undefined }
  | { ref: string; pin: HardhatPin; hardhatPr: PullRequest };

// Resolve the Hardhat ref for a run that didn't name one explicitly. `raw` is
// the pin file's contents, or `undefined` for no pin. The ref is `main` unless
// the pin names a Hardhat PR that is still open. It throws on a malformed pin
// or a missing Hardhat PR, because a fallback to `main` would hide the break
// the pin exists to avoid.
export async function resolveHardhatRef({
  github,
  core,
  raw,
}: {
  github: GitHub;
  core: Core;
  raw: string | undefined;
}): Promise<ResolvedHardhatRef> {
  if (raw === undefined) {
    return { ref: DEFAULT_HARDHAT_REF };
  }

  const pin = parseHardhatPin(raw);
  const prName = pinnedPrName(pin);

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
      `Hardhat compat pin ${prName} has been merged; using Hardhat ` +
        `${DEFAULT_HARDHAT_REF}. ${HARDHAT_PIN_PATH} can now be removed.`
    );
    return { ref: DEFAULT_HARDHAT_REF };
  }
  if (hardhatPr.state === "closed") {
    core.warning(
      `Hardhat compat pin ${prName} was closed without merging; using ` +
        `Hardhat ${DEFAULT_HARDHAT_REF}. Remove or update ${HARDHAT_PIN_PATH}.`
    );
    return { ref: DEFAULT_HARDHAT_REF };
  }
  core.info(`Hardhat compat pin active: ${prName} @ ${pin.sha}`);
  return { ref: pin.sha, pin, hardhatPr };
}
