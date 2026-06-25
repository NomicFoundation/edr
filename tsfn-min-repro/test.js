"use strict";

// Driver: load the bare napi-rs v3 addon and run one mode, then let the process
// exit (env teardown). `node test.js <mode>`, looped in CI. See src/lib.rs.
const path = require("path");
const addon = require(path.join(__dirname, "tsfn_min_repro.node"));

const mode = process.argv[2] || "control";

switch (mode) {
  case "control":
    addon.weakTsfn(() => {});
    break;
  case "objectwrap":
    new addon.PlainWrap();
    break;
  case "objectwrap-tsfn":
    new addon.WrapHoldingTsfn(() => {});
    break;
  case "objectwrap-offthread":
    new addon.WrapOffThreadDrop(() => {});
    break;
  case "called-tsfn":
    addon.calledWeakTsfn(() => {});
    break;
  case "runtime-called-tsfn":
    addon.runtimeCalledTsfn(() => {});
    break;
  case "compat-tsfn":
    addon.compatTsfn(() => {});
    break;
  case "compat-tsfn-called":
    addon.compatTsfnCalled(() => {});
    break;
  case "compat-tsfn-heavy":
    addon.compatTsfnHeavy(() => {});
    break;
  case "heavy-call":
    addon.heavyCall(() => {});
    break;
  default:
    throw new Error("unknown mode: " + mode);
}
