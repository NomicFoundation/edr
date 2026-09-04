// Gate for dwarf-decode-benchmark.yml: read the change estimates criterion
// wrote when it compared the PR side against the `pr-base` baseline, and fail
// on a confident regression.
//
// A benchmark fails only when the lower bound of the 95% confidence interval
// on its mean change exceeds the threshold — when even the optimistic bound is
// a real regression. Runner noise of a few percent per side therefore cannot
// fail an innocent PR. Every expected benchmark must be present: one that
// silently stopped running (renamed id, dropped corpus) would otherwise pass
// by absence.
//
// Run from the repo root after `cargo bench ... -- --baseline pr-base`. The
// criterion directory can be passed as the only argument (default:
// target/criterion).
//
// See README.md for the conventions these scripts follow.

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

// Same-machine rerun noise measured +5.9% on the mean and +2.7% on the lower
// bound, so tighter gates false-alarm on the current sample sizes.
export const THRESHOLD = 0.1;

// `<group>/<function>/<parameter>` ids of the benches CI always runs — the
// committed scenarios corpus in crates/edr_solidity/benches/dwarf_decode.rs.
export const EXPECTED_BENCHMARKS = [
  "dwarf_decode/full_pass/scenarios",
  "dwarf_decode/largest_blob/scenarios",
];

export interface ChangeEstimate {
  /** `<group>/<function>/<parameter>`, as criterion lays the directories out. */
  benchmark: string;
  /** Relative change of the mean; 0.05 is +5%. */
  mean: number;
  lowerBound: number;
  upperBound: number;
}

export interface Verdict {
  /** One report line per benchmark, in id order. */
  lines: string[];
  /** Why the gate failed; empty when it passed. */
  failures: string[];
}

// criterion writes `<criterionDir>/<group>/<function>/<parameter>/change/estimates.json`
// for each benchmark that had a baseline to compare against.
export function readChangeEstimates(criterionDir: string): ChangeEstimate[] {
  if (!existsSync(criterionDir)) {
    throw new Error(`${criterionDir} does not exist - did cargo bench run?`);
  }

  const estimates: ChangeEstimate[] = [];
  for (const benchmark of benchmarkDirectories(criterionDir)) {
    const path = join(criterionDir, benchmark, "change", "estimates.json");
    if (!existsSync(path)) continue;
    estimates.push({ benchmark, ...parseEstimates(path) });
  }

  return estimates.sort((a, b) => a.benchmark.localeCompare(b.benchmark));
}

export function evaluate(
  estimates: ChangeEstimate[],
  {
    threshold = THRESHOLD,
    expected = EXPECTED_BENCHMARKS,
  }: { threshold?: number; expected?: string[] } = {}
): Verdict {
  const lines: string[] = [];
  const failures: string[] = [];

  const present = new Set(estimates.map((estimate) => estimate.benchmark));
  for (const benchmark of expected) {
    if (!present.has(benchmark)) {
      failures.push(
        `${benchmark}: no change estimate found - the bench did not run against the baseline, or its id changed`
      );
    }
  }

  for (const estimate of estimates) {
    const regressed = estimate.lowerBound > threshold;
    const line =
      `${estimate.benchmark}: mean ${percent(estimate.mean)} ` +
      `(95% CI ${percent(estimate.lowerBound)} .. ${percent(estimate.upperBound)})` +
      (regressed
        ? `  <-- regressed more than ${percent(threshold, 0, false)}`
        : "");
    lines.push(line);
    if (regressed) failures.push(line);
  }

  return { lines, failures };
}

// Depth-3 walk over `<group>/<function>/<parameter>`. criterion's own
// `report/` directory is shallower and holds no estimates.
function benchmarkDirectories(criterionDir: string): string[] {
  const benchmarks: string[] = [];
  for (const group of subdirectories(criterionDir)) {
    for (const fn of subdirectories(join(criterionDir, group))) {
      for (const parameter of subdirectories(join(criterionDir, group, fn))) {
        benchmarks.push(`${group}/${fn}/${parameter}`);
      }
    }
  }
  return benchmarks;
}

function subdirectories(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name);
}

function parseEstimates(path: string): Omit<ChangeEstimate, "benchmark"> {
  const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
  const mean = field(parsed, "mean");
  const interval = field(mean, "confidence_interval");
  const estimate = {
    mean: field(mean, "point_estimate"),
    lowerBound: field(interval, "lower_bound"),
    upperBound: field(interval, "upper_bound"),
  };

  for (const [name, value] of Object.entries(estimate)) {
    if (typeof value !== "number" || Number.isNaN(value)) {
      throw new Error(`${path}: ${name} is not a number`);
    }
  }

  return estimate as Omit<ChangeEstimate, "benchmark">;
}

function field(value: unknown, key: string): unknown {
  if (typeof value !== "object" || value === null || !(key in value)) {
    throw new Error(`criterion estimates are missing "${key}"`);
  }
  return (value as Record<string, unknown>)[key];
}

function percent(value: number, digits = 1, signed = true): string {
  const sign = signed && value >= 0 ? "+" : "";
  return `${sign}${(value * 100).toFixed(digits)}%`;
}

function main(): void {
  const criterionDir = process.argv[2] ?? "target/criterion";
  const { lines, failures } = evaluate(readChangeEstimates(criterionDir));

  for (const line of lines) console.log(line);
  if (failures.length > 0) {
    console.error(`\n${failures.length} benchmark(s) failed the gate:`);
    for (const failure of failures) console.error(`  ${failure}`);
    process.exitCode = 1;
  }
}

if (import.meta.main) {
  try {
    main();
  } catch (error: unknown) {
    console.error(error);
    process.exitCode = 1;
  }
}
