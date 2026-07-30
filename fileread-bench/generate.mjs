// Generates a set of test files for the read benchmark.
// Usage: node generate.mjs <dir> <numFiles> <sizeBytes>
import fs from "node:fs";
import path from "node:path";

const dir = process.argv[2] ?? "./data";
const numFiles = Number(process.argv[3] ?? 4000);
const sizeBytes = Number(process.argv[4] ?? 32768);

fs.rmSync(dir, { recursive: true, force: true });
fs.mkdirSync(dir, { recursive: true });

// Build a reusable buffer of pseudo-random-ish printable bytes.
const chunk = Buffer.alloc(sizeBytes);
for (let i = 0; i < sizeBytes; i++) {
  chunk[i] = 33 + ((i * 2654435761) >>> 0) % 94; // printable ASCII
}

for (let i = 0; i < numFiles; i++) {
  fs.writeFileSync(path.join(dir, `${i}.txt`), chunk);
}

console.log(
  `Generated ${numFiles} files of ${sizeBytes} bytes (${(
    (numFiles * sizeBytes) /
    1024 /
    1024
  ).toFixed(1)} MiB total) in ${dir}`,
);
