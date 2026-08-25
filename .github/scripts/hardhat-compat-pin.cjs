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
// Consumed by:
//   - resolve-regression-trigger.cjs: while the Hardhat PR is open, regression
//     benchmark runs that don't name an explicit hardhat-ref use the pinned
//     sha instead of `main`.
//   - validate-hardhat-compat-pin.cjs: CI check that validates the file on
//     PRs that touch it, before the pin is ever consumed.

// The Hardhat repository the pin (and the benchmark) refers to. The pinned PR
// must live here (not in a fork), so the benchmark job can check the sha out
// directly.
const HARDHAT_OWNER = "NomicFoundation";
const HARDHAT_REPO = "hardhat";

// Where the pin lives in the EDR repo.
const HARDHAT_PIN_PATH = ".github/hardhat-compat-pin.json";

// Parse and validate the raw file contents. Returns the pin with a lowercased
// sha; throws a descriptive error on invalid JSON or a malformed shape.
function parseHardhatPin(raw) {
  let pin;
  try {
    pin = JSON.parse(raw);
  } catch (e) {
    throw new Error(`${HARDHAT_PIN_PATH} is not valid JSON: ${e.message}`);
  }
  if (
    !Number.isInteger(pin.pr) ||
    pin.pr <= 0 ||
    typeof pin.sha !== "string" ||
    !/^[0-9a-f]{40}$/i.test(pin.sha)
  ) {
    throw new Error(
      `${HARDHAT_PIN_PATH} must contain {"pr": <Hardhat PR number>, ` +
        `"sha": "<full 40-hex commit sha>"}; got: ${raw.trim()}`
    );
  }
  return { ...pin, sha: pin.sha.toLowerCase() };
}

module.exports = {
  HARDHAT_OWNER,
  HARDHAT_REPO,
  HARDHAT_PIN_PATH,
  parseHardhatPin,
};
