// Unit tests for check-dwarf-benchmark-regression.ts.
//
// Run with Node's built-in test runner (no extra dependencies):
//   node --test .github/scripts/check-dwarf-benchmark-regression.test.ts

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  evaluate,
  EXPECTED_BENCHMARKS,
  readChangeEstimates,
  THRESHOLD,
  type ChangeEstimate,
} from "./check-dwarf-benchmark-regression.ts";

const SCRIPT = join(
  dirname(fileURLToPath(import.meta.url)),
  "check-dwarf-benchmark-regression.ts"
);

const [FULL_PASS, LARGEST_BLOB] = EXPECTED_BENCHMARKS as [string, string];

// A fresh criterion directory with `<benchmark>/change/estimates.json` written
// the way criterion does (mean and median blocks; only mean is read).
function makeCriterionDir(
  changes: Record<string, { mean: number; lower: number; upper: number }>
): string {
  const dir = mkdtempSync(join(tmpdir(), "criterion-"));
  // criterion always writes these alongside the benches; neither holds estimates.
  mkdirSync(join(dir, "report"));
  for (const [benchmark, { mean, lower, upper }] of Object.entries(changes)) {
    writeEstimates(dir, benchmark, criterionEstimates(mean, lower, upper));
  }
  return dir;
}

function writeEstimates(dir: string, benchmark: string, contents: string) {
  const changeDir = join(dir, benchmark, "change");
  mkdirSync(changeDir, { recursive: true });
  writeFileSync(join(changeDir, "estimates.json"), contents);
}

function criterionEstimates(mean: number, lower: number, upper: number) {
  const block = (point: number) => ({
    confidence_interval: {
      confidence_level: 0.95,
      lower_bound: lower,
      upper_bound: upper,
    },
    point_estimate: point,
    standard_error: 0.001,
  });
  return JSON.stringify({ mean: block(mean), median: block(mean) });
}

function estimate(
  benchmark: string,
  mean: number,
  lowerBound: number,
  upperBound: number
): ChangeEstimate {
  return { benchmark, mean, lowerBound, upperBound };
}

const BOTH_IMPROVED = {
  [FULL_PASS]: { mean: -0.659, lower: -0.66, upper: -0.658 },
  [LARGEST_BLOB]: { mean: -0.705, lower: -0.706, upper: -0.704 },
};

test("readChangeEstimates finds every bench with a change block, in id order", () => {
  const dir = makeCriterionDir({
    [LARGEST_BLOB]: { mean: 0.02, lower: 0.01, upper: 0.03 },
    [FULL_PASS]: { mean: -0.1, lower: -0.11, upper: -0.09 },
  });
  // A bench that ran without a baseline has `new/` but no `change/`.
  mkdirSync(join(dir, "dwarf_decode", "full_pass", "aave-v4", "new"), {
    recursive: true,
  });

  assert.deepEqual(readChangeEstimates(dir), [
    estimate(FULL_PASS, -0.1, -0.11, -0.09),
    estimate(LARGEST_BLOB, 0.02, 0.01, 0.03),
  ]);
});

test("readChangeEstimates rejects a missing criterion directory", () => {
  assert.throws(
    () => readChangeEstimates(join(tmpdir(), "criterion-does-not-exist")),
    /does not exist/
  );
});

test("readChangeEstimates rejects malformed estimates, naming what is missing", () => {
  const dir = makeCriterionDir({});
  writeEstimates(dir, FULL_PASS, JSON.stringify({ median: {} }));
  assert.throws(() => readChangeEstimates(dir), /missing "mean"/);

  writeEstimates(
    dir,
    FULL_PASS,
    JSON.stringify({
      mean: {
        point_estimate: "fast",
        confidence_interval: { lower_bound: 0, upper_bound: 0 },
      },
    })
  );
  assert.throws(() => readChangeEstimates(dir), /mean is not a number/);
});

test("evaluate passes when every expected bench improved", () => {
  const { lines, failures } = evaluate([
    estimate(FULL_PASS, -0.659, -0.66, -0.658),
    estimate(LARGEST_BLOB, -0.705, -0.706, -0.704),
  ]);
  assert.deepEqual(failures, []);
  assert.deepEqual(lines, [
    `${FULL_PASS}: mean -65.9% (95% CI -66.0% .. -65.8%)`,
    `${LARGEST_BLOB}: mean -70.5% (95% CI -70.6% .. -70.4%)`,
  ]);
});

test("evaluate gates on the CI lower bound, not the mean", () => {
  const passing = evaluate([
    // A large mean with an interval that still reaches below the threshold
    // is noise, not a confident regression.
    estimate(FULL_PASS, 0.3, 0.05, 0.55),
    estimate(LARGEST_BLOB, 0.12, THRESHOLD - 0.001, 0.14),
  ]);
  assert.deepEqual(passing.failures, []);

  const failing = evaluate([
    estimate(FULL_PASS, 0.0, -0.01, 0.01),
    estimate(LARGEST_BLOB, 0.12, THRESHOLD + 0.001, 0.14),
  ]);
  assert.deepEqual(failing.failures, [
    `${LARGEST_BLOB}: mean +12.0% (95% CI +10.1% .. +14.0%)  <-- regressed more than 10%`,
  ]);
  // The report still lists every bench.
  assert.equal(failing.lines.length, 2);
});

test("evaluate fails when an expected bench has no estimate", () => {
  const { failures } = evaluate([estimate(FULL_PASS, -0.1, -0.11, -0.09)]);
  assert.equal(failures.length, 1);
  assert.match(
    failures[0] ?? "",
    new RegExp(`^${LARGEST_BLOB}: no change estimate found`)
  );

  assert.equal(evaluate([]).failures.length, EXPECTED_BENCHMARKS.length);
});

test("evaluate reports and gates benches beyond the expected set", () => {
  const extra = "dwarf_decode/full_pass/aave-v4";
  const { lines, failures } = evaluate([
    estimate(FULL_PASS, 0.0, -0.01, 0.01),
    estimate(LARGEST_BLOB, 0.0, -0.01, 0.01),
    estimate(extra, 0.2, 0.15, 0.25),
  ]);
  assert.equal(lines.length, 3);
  assert.deepEqual(failures, [
    `${extra}: mean +20.0% (95% CI +15.0% .. +25.0%)  <-- regressed more than 10%`,
  ]);
});

// Run the script as the workflow does, against a criterion directory.
function run(criterionDir: string) {
  return spawnSync(process.execPath, [SCRIPT, criterionDir], {
    encoding: "utf8",
  });
}

test("the script exits 0 and prints the report on a pass", () => {
  const result = run(makeCriterionDir(BOTH_IMPROVED));
  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    result.stdout,
    `${FULL_PASS}: mean -65.9% (95% CI -66.0% .. -65.8%)\n` +
      `${LARGEST_BLOB}: mean -70.5% (95% CI -70.6% .. -70.4%)\n`
  );
});

test("the script exits 1 on a confident regression", () => {
  const result = run(
    makeCriterionDir({
      ...BOTH_IMPROVED,
      [FULL_PASS]: { mean: 0.15, lower: 0.12, upper: 0.18 },
    })
  );
  assert.equal(result.status, 1);
  assert.match(result.stdout, /regressed more than 10%/);
  assert.match(result.stderr, /1 benchmark\(s\) failed the gate/);
});

test("the script exits 1 when the comparison did not run", () => {
  const result = run(makeCriterionDir({}));
  assert.equal(result.status, 1);
  assert.match(result.stderr, /no change estimate found/);
});
