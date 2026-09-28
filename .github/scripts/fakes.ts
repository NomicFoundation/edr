// Fakes shared by the workflow-script tests. The name must not match
// `node --test`'s default patterns (`*.test.ts`, `test-*.ts`, ...), or the
// runner would load it as a test file.

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
