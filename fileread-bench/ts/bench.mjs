// TS/Node file-read benchmark.
// Reads every file in <dir> and sums total bytes, across several strategies.
// Concurrency of async fs is bounded by libuv's worker pool (UV_THREADPOOL_SIZE).
import fs from "node:fs";
import fsp from "node:fs/promises";
import path from "node:path";

const dir = process.argv[2] ?? "../data";
const iters = Number(process.argv[3] ?? 5);

const files = fs
  .readdirSync(dir)
  .filter((f) => f.endsWith(".txt"))
  .map((f) => path.join(dir, f));

function now() {
  return Number(process.hrtime.bigint()) / 1e6; // ms
}

function stats(times) {
  const s = [...times].sort((a, b) => a - b);
  const median = s[Math.floor(s.length / 2)];
  return { min: s[0], median };
}

async function run(label, fn) {
  const times = [];
  let bytes = 0;
  for (let i = 0; i < iters; i++) {
    const t0 = now();
    bytes = await fn();
    times.push(now() - t0);
  }
  const { min, median } = stats(times);
  console.log(
    `${label.padEnd(34)} median=${median.toFixed(1).padStart(8)} ms  min=${min
      .toFixed(1)
      .padStart(8)} ms  (${bytes} bytes)`,
  );
}

// Sequential: await each read one at a time.
async function sequential() {
  let bytes = 0;
  for (const f of files) {
    const buf = await fsp.readFile(f);
    bytes += buf.length;
  }
  return bytes;
}

// Promise.all: fire all reads at once. Actual in-flight syscalls are still
// capped by the libuv thread pool size.
async function promiseAll() {
  const bufs = await Promise.all(files.map((f) => fsp.readFile(f)));
  return bufs.reduce((a, b) => a + b.length, 0);
}

// utf-8 read: adds a main-thread Buffer->string decode on top of the read.
async function promiseAllUtf8() {
  const strs = await Promise.all(files.map((f) => fsp.readFile(f, "utf-8")));
  return strs.reduce((a, b) => a + b.length, 0);
}

// stat only: the doc's other strace example.
async function statAll() {
  const st = await Promise.all(files.map((f) => fsp.stat(f)));
  return st.reduce((a, b) => a + b.size, 0);
}

console.log(
  `\n== Node ${process.version}  files=${files.length}  UV_THREADPOOL_SIZE=${
    process.env.UV_THREADPOOL_SIZE ?? "(default 4)"
  }  iters=${iters} ==`,
);

await run("sequential (await loop)", sequential);
await run("Promise.all (concurrent)", promiseAll);
await run("Promise.all utf-8", promiseAllUtf8);
await run("Promise.all stat", statAll);
