// Print the EDR napi platform-package suffix for the current host (e.g.
// `linux-x64-gnu`, `darwin-arm64`) to stdout, or exit non-zero on an
// unsupported platform. The suffix matches a subdirectory of
// `crates/edr_napi/npm/` and the `@nomicfoundation/edr-<suffix>` package name.
//
// Used by scripts/publish_to_verdaccio.sh.
// See README.md for the conventions these scripts follow.

import { execSync } from "node:child_process";

/** Platform package suffix by `${platform}-${arch}`, with `<libc>` to fill in. */
const SUFFIXES: Record<string, string> = {
  "darwin-arm64": "darwin-arm64",
  "darwin-x64": "darwin-x64",
  "win32-x64": "win32-x64-msvc",
  "linux-x64": "linux-x64-<libc>",
  "linux-arm64": "linux-arm64-<libc>",
};

/**
 * The platform package suffix for a host, or `undefined` if EDR publishes no
 * package for it. `libc` is only consulted on Linux.
 */
export function platformPackageSuffix(
  platform: string,
  arch: string,
  libc: "gnu" | "musl"
): string | undefined {
  return SUFFIXES[`${platform}-${arch}`]?.replace("<libc>", libc);
}

/** Every suffix this detector can print, for cross-checking against `npm/`. */
export function knownPlatformPackageSuffixes(): string[] {
  return Object.values(SUFFIXES).flatMap((suffix) =>
    suffix.includes("<libc>")
      ? [suffix.replace("<libc>", "gnu"), suffix.replace("<libc>", "musl")]
      : [suffix]
  );
}

/** The C library `ldd --version` output describes. Defaults to glibc. */
export function parseLibc(lddVersionOutput: string): "gnu" | "musl" {
  return /musl/i.test(lddVersionOutput) ? "musl" : "gnu";
}

/** Whether this host's C library is musl. Only meaningful on Linux. */
function detectLibc(): "gnu" | "musl" {
  try {
    return parseLibc(
      execSync("ldd --version 2>&1 || true", { encoding: "utf8" })
    );
  } catch {
    return "gnu";
  }
}

if (import.meta.main) {
  const { platform, arch } = process;
  const libc = platform === "linux" ? detectLibc() : "gnu";
  const suffix = platformPackageSuffix(platform, arch, libc);

  if (suffix === undefined) {
    console.error(`Unsupported platform: ${platform}-${arch}`);
    process.exit(1);
  }

  process.stdout.write(suffix);
}
