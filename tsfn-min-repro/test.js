"use strict";

// Minimal driver: load the bare napi-rs v3 addon, register a weak
// ThreadsafeFunction (leaked inside the addon), then let the process exit.
// Because the TSFN is weak it does not keep the event loop alive, so node
// proceeds to env teardown with a still-registered weak global handle — the
// condition that crashes EDR with `Check failed: node->IsInUse()`.
const path = require("path");
const addon = require(path.join(__dirname, "tsfn_min_repro.node"));

addon.registerWeakTsfn(() => {});
