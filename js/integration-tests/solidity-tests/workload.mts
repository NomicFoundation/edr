// Long-running memory trajectory workload for the step-trace fix analysis.
//
// Runs the 16 "Heavy" fuzz suites `--loops` times in ONE process (simulating a
// program that keeps allocating/dropping CallTraceArenas over its lifetime),
// while sampling this process's RSS every 20 ms. Emits the full RSS-over-time
// trajectory (JSON) with markers at setup-done and each suite/loop boundary.
//
// This answers "does the fix net-reduce memory, or just make the graph jagged?"
// — the unfixed binary should climb to a high plateau and stay there (retained
// pages); the fixed binary should saw-tooth (mi_collect returns pages after
// each suite) with a bounded, low envelope.
//
// Usage:
//   node --expose-gc --import tsx/esm workload.mts \
//        --verbosity <3|4> --loops <n> --out <path.json>

import fs from "node:fs";
import { L1_CHAIN_TYPE, CollectStackTraces, IncludeTraces } from "@nomicfoundation/edr";
import { TestContext } from "./test/testContext.js";

function arg(name: string, def: string): string {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && i + 1 < process.argv.length ? process.argv[i + 1] : def;
}

const verbosity = parseInt(arg("verbosity", "3"), 10);
const loops = parseInt(arg("loops", "4"), 10);
const outPath = arg("out", "/tmp/trajectory.json");

function configForVerbosity(v: number) {
  return {
    collectStackTraces: v > 2 ? CollectStackTraces.Always : CollectStackTraces.OnFailure,
    includeTraces:
      v >= 4 ? IncludeTraces.All : v >= 3 ? IncludeTraces.Failing : IncludeTraces.None,
  };
}

interface Sample {
  t: number;
  rssMB: number;
}
interface Marker {
  t: number;
  label: string;
}

async function runBatch(
  ctx: TestContext,
  suites: any[],
  config: any,
  onSuite: () => void,
): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let done = false;
    let cb = 0;
    const tryResolve = () => {
      if (done && cb === suites.length) resolve();
    };
    (ctx.edrContext as any)
      .runSolidityTests(
        L1_CHAIN_TYPE,
        ctx.artifacts,
        suites,
        config,
        ctx.tracingConfig,
        (_suiteResult: any) => {
          // Intentionally do NOT retain the suite result: mimic a streaming
          // consumer so freed arenas become reclaimable per suite.
          onSuite();
          cb++;
          tryResolve();
        },
      )
      .then(() => {
        done = true;
        tryResolve();
      })
      .catch(reject);
  });
}

async function main() {
  const config0 = configForVerbosity(verbosity);
  const ctx = await TestContext.setup();

  const names = new Set(
    Array.from({ length: 16 }, (_, i) => `Heavy${String(i).padStart(2, "0")}Test`),
  );
  const suites = ctx.testSuiteIds.filter((s) => names.has(s.name));
  if (suites.length !== 16) {
    throw new Error(`Expected 16 heavy suites, found ${suites.length}`);
  }
  const config = { ...ctx.defaultConfig(L1_CHAIN_TYPE), ...config0 };

  // Start sampling only now, so the trajectory covers the run phase, not the
  // one-time solc compile during setup.
  const t0 = performance.now();
  const samples: Sample[] = [];
  const markers: Marker[] = [];
  const now = () => performance.now() - t0;
  const sample = () =>
    samples.push({ t: now(), rssMB: process.memoryUsage().rss / (1024 * 1024) });

  sample();
  markers.push({ t: now(), label: "run-start" });
  const timer = setInterval(sample, 20);

  let suiteCount = 0;
  for (let loop = 0; loop < loops; loop++) {
    await runBatch(ctx, suites, config, () => {
      suiteCount++;
      markers.push({ t: now(), label: `suite ${suiteCount}` });
    });
    markers.push({ t: now(), label: `loop ${loop + 1} done` });
  }

  sample();
  clearInterval(timer);

  const cfgLabel = `${config0.collectStackTraces === CollectStackTraces.Always ? "Always" : "OnFailure"}/${
    config0.includeTraces === IncludeTraces.All
      ? "All"
      : config0.includeTraces === IncludeTraces.Failing
        ? "Failing"
        : "None"
  }`;

  fs.writeFileSync(
    outPath,
    JSON.stringify(
      {
        verbosity,
        loops,
        config: cfgLabel,
        suites: suiteCount,
        durationMs: Math.round(now()),
        peakRssMB: Math.max(...samples.map((s) => s.rssMB)),
        finalRssMB: samples[samples.length - 1].rssMB,
        samples,
        markers,
      },
      null,
      2,
    ),
  );
  process.stderr.write(
    `wrote ${outPath}: ${samples.length} samples over ${Math.round(now())}ms, ` +
      `peak ${Math.max(...samples.map((s) => s.rssMB)).toFixed(0)}MB, ` +
      `final ${samples[samples.length - 1].rssMB.toFixed(0)}MB\n`,
  );
  process.exit(0);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
