// Memory measurement harness for the solidity-test step-trace RSS fix.
//
// Runs the 16 "Heavy" fuzz suites through EDR at a given verbosity level and
// reports RSS. One config per process invocation (fresh allocator state).
//
// Usage:
//   node --expose-gc --import tsx/esm measure-mem.mts --verbosity <2|3|4> --mode <hold|stream>
//
// Verbosity -> EDR config (mirrors packages/hardhat .../solidity-test/helpers.ts
// and .../trace-formatters.ts):
//   v<=2 : collectStackTraces=OnFailure, includeTraces=None   (baseline, -vv)
//   v==3 : collectStackTraces=Always,    includeTraces=Failing (-vvv)
//   v>=4 : collectStackTraces=Always,    includeTraces=All     (-vvvv / -vvvvv)
//
// mode:
//   hold   : retain every SuiteResult for the whole run (matches
//            runAllSolidityTests / the ISSUE methodology).
//   stream : drop each SuiteResult immediately after its callback (mimics a
//            reporter that consumes-and-releases per suite).

import fs from "node:fs";
import { L1_CHAIN_TYPE, CollectStackTraces, IncludeTraces } from "@nomicfoundation/edr";
import { TestContext } from "./test/testContext.js";

function arg(name: string, def: string): string {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && i + 1 < process.argv.length ? process.argv[i + 1] : def;
}

const verbosity = parseInt(arg("verbosity", "3"), 10);
const mode = arg("mode", "hold");

function configForVerbosity(v: number) {
  const collectStackTraces =
    v > 2 ? CollectStackTraces.Always : CollectStackTraces.OnFailure;
  const includeTraces =
    v >= 4 ? IncludeTraces.All : v >= 3 ? IncludeTraces.Failing : IncludeTraces.None;
  return { collectStackTraces, includeTraces };
}

function vmHWMbytes(): number {
  // Peak resident set size (kernel high-water mark), whole process lifetime.
  try {
    const status = fs.readFileSync("/proc/self/status", "utf8");
    const m = status.match(/VmHWM:\s+(\d+)\s+kB/);
    return m ? parseInt(m[1], 10) * 1024 : -1;
  } catch {
    return -1;
  }
}

const MB = (b: number) => (b / (1024 * 1024)).toFixed(1);

async function main() {
  const { collectStackTraces, includeTraces } = configForVerbosity(verbosity);

  const ctx = await TestContext.setup();

  // Select the 16 heavy fuzz suites.
  const names = new Set(Array.from({ length: 16 }, (_, i) => `Heavy${String(i).padStart(2, "0")}Test`));
  const suites = ctx.testSuiteIds.filter((s) => names.has(s.name));
  if (suites.length !== 16) {
    throw new Error(`Expected 16 heavy suites, found ${suites.length}: ${suites.map((s) => s.name).join(",")}`);
  }

  const config = {
    ...ctx.defaultConfig(L1_CHAIN_TYPE),
    collectStackTraces,
    includeTraces,
  };

  // Baseline RSS after setup/compile, before the run.
  if (global.gc) global.gc();
  const rssBeforeRun = process.memoryUsage().rss;

  // Poll RSS during the run to capture the run-phase peak.
  let peakDuringRun = rssBeforeRun;
  const poller = setInterval(() => {
    const r = process.memoryUsage().rss;
    if (r > peakDuringRun) peakDuringRun = r;
  }, 25);

  // Counts, for the losslessness check.
  let totalTests = 0;
  let passed = 0;
  let failed = 0;
  let suitesSeen = 0;

  // Retained results (hold mode).
  const held: any[] = [];

  const finalResult: any = await new Promise((resolve, reject) => {
    let done = false;
    let cbCount = 0;
    const tryResolve = (fr?: any) => {
      if (done && cbCount === suites.length) resolve(fr);
    };
    let finalFr: any;
    (ctx.edrContext as any)
      .runSolidityTests(
        L1_CHAIN_TYPE,
        ctx.artifacts,
        suites,
        config,
        ctx.tracingConfig,
        (suiteResult: any) => {
          suitesSeen++;
          for (const tr of suiteResult.testResults) {
            totalTests++;
            if (tr.status === "Success") passed++;
            else failed++;
          }
          if (mode === "hold") {
            held.push(suiteResult);
          }
          // stream mode: intentionally drop the reference here.
          cbCount++;
          tryResolve(finalFr);
        }
      )
      .then((fr: any) => {
        finalFr = fr;
        done = true;
        tryResolve(fr);
      })
      .catch(reject);
  });
  void finalResult;

  clearInterval(poller);

  const rssAfterRun = process.memoryUsage().rss;

  // Drop retained references and force GC to expose JS-side live retention.
  held.length = 0;
  if (global.gc) {
    global.gc();
    global.gc();
  }
  // Small delay so any deferred deallocation settles.
  await new Promise((r) => setTimeout(r, 300));
  if (global.gc) global.gc();
  const rssAfterGC = process.memoryUsage().rss;
  const peakVmHWM = vmHWMbytes();

  const out = {
    verbosity,
    mode,
    collectStackTraces: collectStackTraces === CollectStackTraces.Always ? "Always" : "OnFailure",
    includeTraces:
      includeTraces === IncludeTraces.All ? "All" : includeTraces === IncludeTraces.Failing ? "Failing" : "None",
    suitesSeen,
    totalTests,
    passed,
    failed,
    gcAvailable: Boolean(global.gc),
    rssBeforeRunMB: Number(MB(rssBeforeRun)),
    rssAfterRunMB: Number(MB(rssAfterRun)),
    rssAfterGCMB: Number(MB(rssAfterGC)),
    peakDuringRunMB: Number(MB(peakDuringRun)),
    peakVmHWM_MB: Number(MB(peakVmHWM)),
  };
  // Machine-readable line for the driver, plus a human line.
  console.log("RESULT_JSON " + JSON.stringify(out));
  console.log(
    `v${verbosity} ${mode} | ${out.collectStackTraces}/${out.includeTraces} | ` +
      `suites=${suitesSeen} tests=${totalTests} pass=${passed} fail=${failed} | ` +
      `rss before=${out.rssBeforeRunMB} afterRun=${out.rssAfterRunMB} afterGC=${out.rssAfterGCMB} ` +
      `peakRun=${out.peakDuringRunMB} VmHWM=${out.peakVmHWM_MB} MB`
  );
}

main().then(
  () => process.exit(0),
  (e) => {
    console.error(e);
    process.exit(1);
  }
);
