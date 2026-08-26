// Types and helpers shared by the scripts `actions/github-script` loads, and by
// their tests. Types are erased at run time, so importing them costs nothing;
// the two helpers below are the only values this module contributes.
//
// The `github` client slices stay in the modules that use them. Each script
// touches a different set of endpoints, and a merged interface would force
// every mock to stub endpoints its script never calls.
//
// See README.md for the conventions these scripts follow.

/** The `core` methods these scripts use. */
export interface Core {
  info: (message: string) => void;
  notice: (message: string) => void;
  warning: (message: string) => void;
}

/** `core` for a script that also sets step outputs. */
export interface CoreWithOutputs extends Core {
  setOutput: (name: string, value: string) => void;
}

/** The `actions.listWorkflowRuns` fields these scripts read. */
export interface WorkflowRun {
  id: number;
  status: string;
  conclusion: string | null;
}

/** The `pulls.get` fields these scripts read. */
export interface PullRequest {
  merged: boolean;
  state: string;
  head: { sha: string };
}

// Octokit rejects with an error carrying an HTTP `status`, and `fs` with a
// `code`. Neither is typed, so read them defensively.

export function errorStatus(e: unknown): number | undefined {
  return typeof e === "object" &&
    e !== null &&
    "status" in e &&
    typeof e.status === "number"
    ? e.status
    : undefined;
}

export function errorCode(e: unknown): string | undefined {
  return typeof e === "object" &&
    e !== null &&
    "code" in e &&
    typeof e.code === "string"
    ? e.code
    : undefined;
}
