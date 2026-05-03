import { Worker } from "worker_threads";

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
const WATCHDOG_HEARTBEAT_MS = 5_000;
const WATCHDOG_STALL_MS = 30_000;
const watchdogSource = `
  const { parentPort } = require("worker_threads");
  let lastBeat = Date.now();
  let lastWarning = 0;
  parentPort.on("message", (msg) => {
    if (msg && msg.type === "beat") lastBeat = Date.now();
  });
  setInterval(() => {
    const silentMs = Date.now() - lastBeat;
    if (silentMs >= ${WATCHDOG_STALL_MS} && Date.now() - lastWarning >= ${WATCHDOG_STALL_MS}) {
      console.error("[mocha-watchdog] main-thread silent for " + Math.round(silentMs / 1000) + "s");
      lastWarning = Date.now();
    }
  }, ${WATCHDOG_HEARTBEAT_MS}).unref();
`;
const watchdog = new Worker(watchdogSource, { eval: true });
watchdog.unref();
const heartbeat = setInterval(() => {
  watchdog.postMessage({ type: "beat" });
}, WATCHDOG_HEARTBEAT_MS);
heartbeat.unref();

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
