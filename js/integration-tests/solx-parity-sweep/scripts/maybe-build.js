// The sweep skips itself when `@nomicfoundation/hardhat-slang-solx` is not
// installed. When the suite is going to skip, running the workspace's full
// `pnpm build:dev` from `pretest` is wasted CI time. Detect the plugin up
// front and only run the build when it actually has work to do.
//
// TODO: once the workspace's hardhat satisfies the plugin's peer range
// (^3.15.0), add `@nomicfoundation/hardhat-slang-solx` to `devDependencies`
// and delete this script — `pretest` can then just call `pnpm build:dev`
// directly.

import { execSync } from "node:child_process";
import { resolve } from "node:path";

let pluginAvailable = false;
try {
  await import("@nomicfoundation/hardhat-slang-solx");
  pluginAvailable = true;
} catch {
  // plugin missing — sweep will skip; nothing to build.
}

if (!pluginAvailable) {
  console.log(
    "[solx-parity-sweep] hardhat-slang-solx not installed; skipping pretest build."
  );
  process.exit(0);
}

const sweepRoot = resolve(import.meta.dirname, "..");
const repoRoot = resolve(sweepRoot, "..", "..", "..");

execSync("pnpm build:dev", { cwd: repoRoot, stdio: "inherit" });
