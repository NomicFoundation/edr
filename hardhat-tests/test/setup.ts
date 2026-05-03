import chai from "chai";
import chaiAsPromised from "chai-as-promised";
import chalk from "chalk";
import { Worker } from "worker_threads";

chai.use(chaiAsPromised);

function getEnv(key: string): string | undefined {
  const variable = process.env[key];
  if (variable === undefined || variable === "") {
    return undefined;
  }

  const trimmed = variable.trim();

  return trimmed.length === 0 ? undefined : trimmed;
}

export const INFURA_URL = getEnv("INFURA_URL");
export const ALCHEMY_URL = getEnv("ALCHEMY_URL");

function printForkingLogicNotBeingTestedWarning(varName: string) {
  console.warn(
    chalk.yellow(
      `TEST RUN INCOMPLETE: You need to define the env variable ${varName}`
    )
  );
}

if (INFURA_URL === undefined) {
  printForkingLogicNotBeingTestedWarning("INFURA_URL");
}

if (ALCHEMY_URL === undefined) {
  printForkingLogicNotBeingTestedWarning("ALCHEMY_URL");
}

// Probe 1: log every test the moment it starts. The default mocha spec
// reporter only prints a name on completion, so when the loop wedges mid-test
// the log gives no clue *which* test was running. The last `[mocha-start]`
// line in CI output now answers that definitively.
beforeEach(function () {
  // eslint-disable-next-line no-console
  console.error(
    `[mocha-start] ${this.currentTest?.fullTitle() ?? "(no test)"}`
  );
});

// Probe 2: heartbeat watchdog running on a Worker thread. The recurring hang
// looks like a hard JS-event-loop jam (mocha's per-test setTimeout never
// fires), so any diagnostic that runs on the main loop is silenced. A worker
// has its own loop and keeps printing even when the main thread is wedged.
//
// First instrumented run produced no watchdog output during a 21-min hang,
// so this iteration is self-instrumenting: lifecycle handlers prove whether
// the worker even started, the worker logs on startup, and the main thread
// emits a periodic alive line independent of the worker so we can tell
// whether the JS loop itself is moving.
const WATCHDOG_HEARTBEAT_MS = 5_000;
const WATCHDOG_STALL_MS = 30_000;
const MAIN_ALIVE_MS = 10_000;
const watchdogSource = `
  const { parentPort } = require("worker_threads");
  console.error("[mocha-watchdog] worker started, pid=" + process.pid);
  let lastBeat = Date.now();
  let lastWarning = 0;
  let beatCount = 0;
  parentPort.on("message", (msg) => {
    if (msg && msg.type === "beat") {
      lastBeat = Date.now();
      beatCount++;
    }
  });
  setInterval(() => {
    const silentMs = Date.now() - lastBeat;
    if (silentMs >= ${WATCHDOG_STALL_MS} && Date.now() - lastWarning >= ${WATCHDOG_STALL_MS}) {
      console.error("[mocha-watchdog] main-thread silent for " + Math.round(silentMs / 1000) + "s (beats=" + beatCount + ")");
      lastWarning = Date.now();
    }
  }, ${WATCHDOG_HEARTBEAT_MS}).unref();
`;
let watchdog: Worker | null = null;
try {
  watchdog = new Worker(watchdogSource, { eval: true });
  watchdog.on("error", (err) => {
    // eslint-disable-next-line no-console
    console.error(`[mocha-watchdog] worker error: ${err.message}`);
  });
  watchdog.on("exit", (code) => {
    // eslint-disable-next-line no-console
    console.error(`[mocha-watchdog] worker exited code=${code}`);
  });
  watchdog.on("online", () => {
    // eslint-disable-next-line no-console
    console.error("[mocha-watchdog] worker online");
  });
  watchdog.unref();
} catch (err) {
  // eslint-disable-next-line no-console
  console.error(
    `[mocha-watchdog] failed to spawn worker: ${(err as Error).message}`
  );
}

const heartbeat = setInterval(() => {
  watchdog?.postMessage({ type: "beat" });
}, WATCHDOG_HEARTBEAT_MS);
heartbeat.unref();

// Probe 2b: main-thread liveness ping. Independent of the worker — if these
// lines stop appearing, the JS event loop itself is wedged. If they keep
// appearing through a hang, the loop is alive but mocha's per-test timer is
// somehow disabled or a NAPI promise isn't resolving.
const mainAlive = setInterval(() => {
  // eslint-disable-next-line no-console
  console.error(`[mocha-main-alive] uptime=${Math.round(process.uptime())}s`);
}, MAIN_ALIVE_MS);
mainAlive.unref();

// Probe 3 (existing): if the loop *is* still functional but a handle is
// keeping the process alive after the suite ends, dump the active resources
// 5s after the last `after` and force-exit so CI doesn't burn the timeout.
after(function () {
  setTimeout(() => {
    const active =
      typeof (process as any).getActiveResourcesInfo === "function"
        ? (process as any).getActiveResourcesInfo()
        : [];
    // eslint-disable-next-line no-console
    console.error(
      `[mocha-exit-diagnostic] process still alive after suite; active resources (${active.length}): ${JSON.stringify(active)}`
    );
    process.exit(0);
  }, 5000).unref();
});
