// Run hardhat tests under both build profiles and compare per-test traces.
// Parity = same Error reason, same frame count, same per-frame
// `Contract.function` + `file:line`. Known divergences are pinned as
// goldens in `scenariosDivergingFromSolc` so improvements force an update.

import { spawnSync, type SpawnSyncReturns } from "node:child_process";
import { describe, it, before } from "node:test";
import assert from "node:assert/strict";

interface Frame {
  location: string;
  file: string | null;
}

interface Block {
  contract: string;
  test: string;
  reason: string | null;
  frames: Frame[];
}

function runProfile(args: string[]): SpawnSyncReturns<string> {
  return spawnSync("pnpm", ["hardhat", "test", ...args], {
    cwd: import.meta.dirname + "/..",
    encoding: "utf8",
    env: { ...process.env, FORCE_COLOR: "0", NO_COLOR: "1" },
  });
}

function parseTraceBlocks(output: string): Map<string, Block> {
  const blocks = new Map<string, Block>();
  const lines = output.split("\n");
  let current: Block | null = null;
  // `\([^)]*\)` covers both `()` and fuzz signatures like `(uint256)`.
  const headerRe = /^\s+\d+\)\s+(\w+)#(\w+)\([^)]*\)\s*$/;
  // eslint-disable-next-line no-control-regex
  const stripAnsi = (s: string) => s.replace(/\x1B\[[0-9;]*[A-Za-z]/g, "");
  for (const raw of lines) {
    const line = stripAnsi(raw);
    const headerMatch = line.match(headerRe);
    if (headerMatch !== null) {
      if (current !== null) {
        blocks.set(`${current.contract}#${current.test}`, current);
      }
      current = {
        contract: headerMatch[1],
        test: headerMatch[2],
        reason: null,
        frames: [],
      };
      continue;
    }
    if (current === null) continue;
    const reasonMatch = line.match(/^\s+Error:\s+(.+?)\s*$/);
    if (reasonMatch !== null && current.reason === null) {
      current.reason = reasonMatch[1];
      continue;
    }
    const frameMatch = line.match(/^\s+at\s+(.+?)(?:\s+\((.+?)\))?\s*$/);
    if (frameMatch !== null) {
      current.frames.push({
        location: frameMatch[1],
        file: frameMatch[2] ?? null,
      });
      continue;
    }
    if (line.match(/^\s+(Stack Trace Warning:|Test run failed)/) !== null) {
      current.frames.push({ location: line.trim(), file: null });
      continue;
    }
    // End of block when we hit a blank gap and we already saw frames.
    if (line.trim() === "" && current.frames.length > 0) {
      blocks.set(`${current.contract}#${current.test}`, current);
      current = null;
    }
  }
  if (current !== null) {
    blocks.set(`${current.contract}#${current.test}`, current);
  }
  return blocks;
}

function compare(solc: Block | undefined, solx: Block | undefined): string[] {
  const probs: string[] = [];
  if (solc === undefined) return ["solc trace missing"];
  if (solx === undefined) return ["solx trace missing"];
  if ((solc.reason ?? "") !== (solx.reason ?? "")) {
    probs.push(
      `reason mismatch:\n  solc: ${solc.reason}\n  solx: ${solx.reason}`
    );
  }
  if (solc.frames.length !== solx.frames.length) {
    probs.push(
      `frame count mismatch: solc=${solc.frames.length} solx=${solx.frames.length}`
    );
  }
  const minLen = Math.min(solc.frames.length, solx.frames.length);
  for (let i = 0; i < minLen; i += 1) {
    const a = solc.frames[i];
    const b = solx.frames[i];
    if (a.location !== b.location) {
      probs.push(
        `frame ${i} location mismatch:\n  solc: ${a.location}\n  solx: ${b.location}`
      );
    }
    if ((a.file ?? "") !== (b.file ?? "")) {
      probs.push(
        `frame ${i} file mismatch:\n  solc: ${a.file}\n  solx: ${b.file}`
      );
    }
  }
  return probs;
}

// hardhat-solx is an optional dep — skip the sweep if it's not installed.
let hardhatSolxAvailable = false;
try {
  await import("@nomicfoundation/hardhat-solx");
  hardhatSolxAvailable = true;
} catch {
  // optional dep missing — sweep skipped.
}

describe("solx-vs-solc trace parity", { skip: !hardhatSolxAvailable }, () => {
  let solcBlocks: Map<string, Block>;
  let solxBlocks: Map<string, Block>;
  let allKeys: string[];

  before(() => {
    const solcRun = runProfile([]);
    solcBlocks = parseTraceBlocks(solcRun.stdout);

    const solxRun = runProfile(["--build-profile", "solx"]);
    solxBlocks = parseTraceBlocks(solxRun.stdout);

    allKeys = [...new Set([...solcBlocks.keys(), ...solxBlocks.keys()])].sort();
  });

  // Pinned solx outputs that diverge from solc. Improvements or regressions
  // both surface as golden mismatches; rejoin the parity check by removing
  // an entry once solx matches solc.
  const scenariosDivergingFromSolc = new Map<string, Block>([
    // No `.debug_line` rows for assembly opcodes — bottom frame is the
    // function decl line (129), not solc's statement line (135).
    [
      "InlineAssemblyRevertTest#testInlineAssemblyRevert",
      {
        contract: "InlineAssemblyRevertTest",
        test: "testInlineAssemblyRevert",
        reason: "asmbe",
        frames: [
          {
            location: "InlineAssemblyRevertTest.testInlineAssemblyRevert",
            file: "contracts/Scenarios.t.sol:129",
          },
        ],
      },
    ],
    // Same as InlineAssemblyRevert, but for `invalid()`: 182 vs 183.
    [
      "InvalidOpcodeTest#testInvalidOpcode",
      {
        contract: "InvalidOpcodeTest",
        test: "testInvalidOpcode",
        reason: "EvmError: InvalidFEOpcode",
        frames: [
          {
            location: "InvalidOpcodeTest.testInvalidOpcode",
            file: "contracts/Scenarios.t.sol:182",
          },
        ],
      },
    ],
    // Optimizer unrolls 3-deep self-recursion; inlined frames collapse.
    [
      "InternalRecurseTest#testInternalRecurse",
      {
        contract: "InternalRecurseTest",
        test: "testInternalRecurse",
        reason: "internal bottom",
        frames: [
          {
            location: "InternalRecurseTest.recurseInternal",
            file: "contracts/Scenarios.t.sol:348",
          },
          {
            location: "InternalRecurseTest.testInternalRecurse",
            file: "contracts/Scenarios.t.sol:354",
          },
        ],
      },
    ],
    // Outer test-entry frame resolves to `internal@<pc>` because solx
    // emits no subprogram for the test contract's dispatch PC.
    [
      "MutualRecursionTest#testMutualRecursion",
      {
        contract: "MutualRecursionTest",
        test: "testMutualRecursion",
        reason: "mutual bottom",
        frames: [
          { location: "MutualA.pingA", file: "contracts/Scenarios.t.sol:377" },
          { location: "MutualB.pingB", file: "contracts/Scenarios.t.sol:386" },
          { location: "MutualA.pingA", file: "contracts/Scenarios.t.sol:378" },
          { location: "MutualB.pingB", file: "contracts/Scenarios.t.sol:386" },
          { location: "MutualA.pingA", file: "contracts/Scenarios.t.sol:378" },
          {
            location: "MutualRecursionTest.internal@270",
            file: "contracts/Scenarios.t.sol",
          },
        ],
      },
    ],
    // Modifier body flattened into its function — 2 frames vs solc's 3.
    [
      "NestedModifierRevertTest#testRevertInModifierBody",
      {
        contract: "NestedModifierRevertTest",
        test: "testRevertInModifierBody",
        reason: "unlucky",
        frames: [
          {
            location: "NestedModifierTarget.bumpIfValid",
            file: "contracts/Scenarios.t.sol:420",
          },
          {
            location: "NestedModifierRevertTest.testRevertInModifierBody",
            file: "contracts/Scenarios.t.sol:428",
          },
        ],
      },
    ],
  ]);

  it("compiles both profiles and produces failing-test trace blocks", () => {
    assert.notStrictEqual(solcBlocks.size, 0, "solc produced no trace blocks");
    assert.notStrictEqual(solxBlocks.size, 0, "solx produced no trace blocks");
  });

  it("every scenario from solc has a matching solx block", () => {
    const missing: string[] = [];
    for (const key of solcBlocks.keys()) {
      if (!solxBlocks.has(key)) missing.push(key);
    }
    assert.deepStrictEqual(missing, [], "solx is missing scenarios");
  });

  // Parity: same reason + frame count + each frame's name + file:line.
  it("all non-diverging scenarios match between solc and solx", () => {
    const failures: { key: string; probs: string[] }[] = [];
    for (const key of allKeys) {
      if (scenariosDivergingFromSolc.has(key)) continue;
      const probs = compare(solcBlocks.get(key), solxBlocks.get(key));
      if (probs.length !== 0) failures.push({ key, probs });
    }
    if (failures.length !== 0) {
      const report = failures
        .map(({ key, probs }) => `${key}\n  ${probs.join("\n  ")}`)
        .join("\n\n");
      throw new Error(`Parity failures:\n\n${report}`);
    }
  });

  // Golden: pin each divergence's current shape. A failure means solx
  // changed — investigate, then either remove the entry or update it.
  it("diverging scenarios produce the expected solx output (golden)", () => {
    const failures: { key: string; probs: string[] }[] = [];
    for (const [key, expected] of scenariosDivergingFromSolc) {
      const actual = solxBlocks.get(key);
      if (actual === undefined) {
        failures.push({ key, probs: ["solx produced no block at all"] });
        continue;
      }
      const probs: string[] = [];
      if ((expected.reason ?? "") !== (actual.reason ?? "")) {
        probs.push(
          `reason mismatch:\n  expected: ${expected.reason}\n  actual:   ${actual.reason}`
        );
      }
      if (expected.frames.length !== actual.frames.length) {
        probs.push(
          `frame count mismatch: expected=${expected.frames.length} actual=${actual.frames.length}`
        );
      }
      const minLen = Math.min(expected.frames.length, actual.frames.length);
      for (let i = 0; i < minLen; i += 1) {
        const e = expected.frames[i];
        const a = actual.frames[i];
        if (e.location !== a.location) {
          probs.push(
            `frame ${i} location mismatch:\n  expected: ${e.location}\n  actual:   ${a.location}`
          );
        }
        if ((e.file ?? "") !== (a.file ?? "")) {
          probs.push(
            `frame ${i} file mismatch:\n  expected: ${e.file}\n  actual:   ${a.file}`
          );
        }
      }
      if (probs.length !== 0) failures.push({ key, probs });
    }
    if (failures.length !== 0) {
      const report = failures
        .map(({ key, probs }) => `${key}\n  ${probs.join("\n  ")}`)
        .join("\n\n");
      throw new Error(
        `Golden mismatches in scenariosDivergingFromSolc — solx output drifted from the pinned expectation. If solx improved and now matches solc, remove the entry; if it regressed, investigate before updating the golden.\n\n${report}`
      );
    }
  });
});
