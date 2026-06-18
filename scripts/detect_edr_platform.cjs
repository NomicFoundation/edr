#!/usr/bin/env node
//
// Print the EDR napi platform-package suffix for the current host (e.g.
// `linux-x64-gnu`, `darwin-arm64`) to stdout, or exit non-zero on an
// unsupported platform. The suffix matches a subdirectory of
// `crates/edr_napi/npm/` and the `@nomicfoundation/edr-<suffix>` package name.
//
// Used by scripts/publish_to_verdaccio.sh.

const { platform, arch } = process;

let libc = "";
if (platform === "linux") {
  try {
    const out = require("child_process").execSync("ldd --version 2>&1 || true", {
      encoding: "utf8",
    });
    libc = /musl/i.test(out) ? "musl" : "gnu";
  } catch {
    libc = "gnu";
  }
}

const map = {
  "darwin-arm64": "darwin-arm64",
  "darwin-x64": "darwin-x64",
  "win32-x64": "win32-x64-msvc",
  "linux-x64": `linux-x64-${libc}`,
  "linux-arm64": `linux-arm64-${libc}`,
};

const suffix = map[`${platform}-${arch}`];
if (suffix === undefined) {
  console.error(`Unsupported platform: ${platform}-${arch}`);
  process.exit(1);
}

process.stdout.write(suffix);
