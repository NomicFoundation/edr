// Write a NAPI-RS config listing only the target that builds a given platform
// package, for `napi pre-publish --config-path`.
//
// @napi-rs/cli >= 3.8 validates during pre-publish that every target in
// `napi.targets` has its .node binary staged, and there is no flag to skip
// that. A local build only produces one platform's binary, so the Verdaccio
// publish narrows the target list to that platform instead.
//
// The target is looked up in `napi.targets` rather than hardcoded here, so
// adding a platform to edr_napi needs no change to this script.
//
// Usage:
//   node scripts/write_napi_host_config.ts <napi-dir> <platform-suffix> <out-path>
//
// where <platform-suffix> is a platform package suffix as printed by
// scripts/detect_edr_platform.ts (e.g. `linux-x64-gnu`).
//
// Used by scripts/publish_to_verdaccio.sh.
// See README.md for the conventions these scripts follow.

import { writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

export interface ParsedTriple {
  platformArchABI: string;
}

export type ParseTriple = (target: string) => ParsedTriple;

/**
 * Return the napi target (a Rust triple) that builds the platform package with
 * the given suffix, or `undefined` if no configured target does.
 */
export function findTargetForPlatform(
  targets: readonly string[],
  platformSuffix: string,
  parseTriple: ParseTriple
): string | undefined {
  return targets.find(
    (target) => parseTriple(target).platformArchABI === platformSuffix
  );
}

/** Serialize the NAPI-RS config that narrows `napi.targets` to one target. */
export function napiHostConfig(target: string): string {
  return `${JSON.stringify({ targets: [target] })}\n`;
}

/**
 * Load @napi-rs/cli and `napi.targets` from the package under `napiDir`.
 *
 * Resolves from the package that depends on @napi-rs/cli, not from this
 * script's own location, so the lookup uses the same CLI version that
 * pre-publish will run.
 */
export function loadNapiConfig(napiDir: string): {
  parseTriple: ParseTriple;
  targets: readonly string[];
} {
  const packageJsonPath = join(napiDir, "package.json");
  const napiRequire = createRequire(packageJsonPath);

  // Both requires return `any`, so check the shapes we rely on rather than
  // failing later with `parseTriple is not a function` from inside a callback.
  const { parseTriple }: { parseTriple: ParseTriple } =
    napiRequire("@napi-rs/cli");

  if (typeof parseTriple !== "function") {
    throw new Error(
      `@napi-rs/cli resolved from ${napiDir} does not export parseTriple`
    );
  }

  const { napi }: { napi: { targets: string[] } } =
    napiRequire("./package.json");

  if (!Array.isArray(napi?.targets)) {
    throw new Error(`${packageJsonPath} does not configure napi.targets`);
  }

  return { parseTriple, targets: napi.targets };
}

function main(napiDir: string, platformSuffix: string, outPath: string): void {
  const { parseTriple, targets } = loadNapiConfig(napiDir);

  const target = findTargetForPlatform(targets, platformSuffix, parseTriple);

  if (target === undefined) {
    console.error(
      `error: no napi target builds the ${platformSuffix} platform package`
    );
    console.error(`       napi.targets: ${targets.join(", ")}`);

    process.exit(1);
  }

  writeFileSync(outPath, napiHostConfig(target));
}

// Only run when executed directly, so the tests can import the functions above.
if (import.meta.main) {
  const [napiDir, platformSuffix, outPath] = process.argv.slice(2);

  if (
    napiDir === undefined ||
    platformSuffix === undefined ||
    outPath === undefined
  ) {
    console.error(
      "usage: node scripts/write_napi_host_config.ts <napi-dir> <platform-suffix> <out-path>"
    );

    process.exit(1);
  }

  main(napiDir, platformSuffix, outPath);
}
