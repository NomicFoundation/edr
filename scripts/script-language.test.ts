// Both script directories are TypeScript-only; see scripts/README.md. This is
// a ratchet, not a repo-wide ban: JavaScript scripts elsewhere get converted
// when someone next edits them, and tool config stays CommonJS on purpose.

import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

const TYPESCRIPT_ONLY = ["scripts", join(".github", "scripts")];

// Shell scripts, JSON and Markdown live in these directories too; only
// JavaScript is out of place.
const JAVASCRIPT = /\.(?:c|m)?js$/;

describe("script directories", () => {
  for (const dir of TYPESCRIPT_ONLY) {
    const entries = readdirSync(join(REPO_ROOT, dir), {
      recursive: true,
      withFileTypes: true,
    });

    it(`${dir} has scripts to check`, () => {
      assert.ok(
        entries.some((entry) => entry.name.endsWith(".ts")),
        `no TypeScript found in ${dir}; has it moved?`
      );
    });

    it(`${dir} contains no JavaScript`, () => {
      const javascript = entries
        .filter((entry) => entry.isFile() && JAVASCRIPT.test(entry.name))
        .map((entry) => entry.name);

      assert.deepEqual(
        javascript,
        [],
        `write these as TypeScript instead (see scripts/README.md): ${javascript.join(", ")}`
      );
    });
  }
});
