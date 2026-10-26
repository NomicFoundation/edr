// Fakes shared by the workflow-script tests. The name must not match
// `node --test`'s default patterns (`*.test.ts`, `test-*.ts`, ...), or the
// runner would load it as a test file.

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import type { WorkflowRun } from "./github-script.ts";
import type { Clock } from "./wait-for-workflow-run.ts";

/** A clock that only `sleep` advances, so timeouts cost no wall-clock time. */
export function fakeClock(): Clock {
  let time = 0;
  return {
    sleep: async (ms) => {
      time += ms;
    },
    now: () => time,
  };
}

/** Returns the responses in order on each call, then repeats the last one. */
export function sequence<T>(responses: readonly T[]): () => T {
  if (responses.length === 0) {
    throw new Error("sequence() needs at least one response");
  }
  let calls = 0;
  return () => responses[Math.min(calls++, responses.length - 1)] as T;
}

/**
 * One `listWorkflowRuns` page: a single run stands for a one-run page,
 * `undefined` for an empty one.
 */
export type RunPage = WorkflowRun | WorkflowRun[] | undefined;

export function runsOf(page: RunPage): WorkflowRun[] {
  if (page === undefined) {
    return [];
  }
  return Array.isArray(page) ? page : [page];
}

// A well-formed compat pin, as the file content and as its parts.
export const PIN_SHA = "a".repeat(40);
export const PIN_PR = 5678;
export const PIN_REASON = "needs hardhat counterpart";
export const PIN_FILE = JSON.stringify({
  pr: PIN_PR,
  sha: PIN_SHA,
  reason: PIN_REASON,
});

/**
 * The path a pin would have in a fresh temp dir. `unreadable` puts a
 * directory in its place, which reads as EISDIR rather than ENOENT.
 */
export function tempPinPath({ unreadable = false } = {}): string {
  const dir = mkdtempSync(join(tmpdir(), "compat-pin-"));
  return unreadable ? dir : join(dir, "hardhat-compat-pin.json");
}
