// Shared definition of the Hardhat compatibility pin.
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

// The Hardhat repository the pin (and the benchmark) refers to. The pinned PR
// must live here (not in a fork), so the benchmark job can check the sha out
// directly.
export const HARDHAT_OWNER = "NomicFoundation";
export const HARDHAT_REPO = "hardhat";

// Where the pin lives in the EDR repo.
export const HARDHAT_PIN_PATH = ".github/hardhat-compat-pin.json";

export interface HardhatPin {
  pr: number;
  sha: string;
  // Free-form and never validated, so it is whatever the file contained.
  reason?: unknown;
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
