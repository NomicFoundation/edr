import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  knownPlatformPackageSuffixes,
  parseLibc,
  platformPackageSuffix,
} from "./detect_edr_platform.ts";

const SCRIPTS_DIR = dirname(fileURLToPath(import.meta.url));
const SCRIPT = join(SCRIPTS_DIR, "detect_edr_platform.ts");
const NAPI_DIR = join(SCRIPTS_DIR, "..", "crates", "edr_napi");

// At module scope: a throw inside a `describe` body that has not registered a
// subtest yet is reported but still exits 0.
const knownSuffixes = knownPlatformPackageSuffixes();

const platformSuffixes = readdirSync(join(NAPI_DIR, "npm"), {
  withFileTypes: true,
})
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name);

describe("platformPackageSuffix", () => {
  it("maps the platforms EDR publishes for", () => {
    assert.equal(platformPackageSuffix("linux", "x64", "gnu"), "linux-x64-gnu");
    assert.equal(
      platformPackageSuffix("linux", "arm64", "musl"),
      "linux-arm64-musl"
    );
    assert.equal(
      platformPackageSuffix("darwin", "arm64", "gnu"),
      "darwin-arm64"
    );
    assert.equal(
      platformPackageSuffix("win32", "x64", "gnu"),
      "win32-x64-msvc"
    );
  });

  it("ignores libc off Linux", () => {
    assert.equal(
      platformPackageSuffix("darwin", "x64", "musl"),
      platformPackageSuffix("darwin", "x64", "gnu")
    );
  });

  it("returns undefined for a platform EDR does not publish for", () => {
    assert.equal(platformPackageSuffix("linux", "riscv64", "gnu"), undefined);
    assert.equal(platformPackageSuffix("freebsd", "x64", "gnu"), undefined);
    assert.equal(platformPackageSuffix("win32", "arm64", "gnu"), undefined);
  });
});

// publish_to_verdaccio.sh feeds this detector's output to
// write_napi_host_config.ts, which looks it up in `napi.targets`. That makes
// this map a third list that has to stay in step with `npm/` and `napi.targets`
// — a platform added to the other two but not here is undetectable on that host.
describe("detectable platforms", () => {
  it("has platforms on both sides to compare", () => {
    assert.ok(knownSuffixes.length > 0, "detector maps no platforms");
    assert.ok(platformSuffixes.length > 0, "no npm/ platform packages found");
  });

  for (const suffix of knownSuffixes) {
    it(`has an npm/ package for ${suffix}`, () => {
      assert.ok(
        platformSuffixes.includes(suffix),
        `detector can print ${suffix}, which has no crates/edr_napi/npm/ directory; found: ${platformSuffixes.join(", ")}`
      );
    });
  }

  for (const suffix of platformSuffixes) {
    it(`is detectable for ${suffix}`, () => {
      assert.ok(
        knownSuffixes.includes(suffix),
        `npm/${suffix} exists but no host maps to it in detect_edr_platform.ts`
      );
    });
  }
});

// publish_to_verdaccio.sh substitutes this script's stdout directly into a path
// and a package name, so anything else on stdout — or nothing at all — silently
// corrupts the publish rather than failing it.
describe("CLI", () => {
  it("prints exactly one known suffix, unterminated", () => {
    const result = spawnSync(process.execPath, [SCRIPT], { encoding: "utf8" });

    assert.equal(result.status, 0, result.stderr);
    assert.ok(
      knownSuffixes.includes(result.stdout),
      `printed ${JSON.stringify(result.stdout)}, which is not one of: ${knownSuffixes.join(", ")}`
    );
  });
});

// parseLibc being right is useless if the entrypoint doesn't consult it, so
// drive the real `ldd` lookup with a fake one on PATH.
describe("libc detection", () => {
  it(
    "reports musl when ldd says musl",
    { skip: process.platform !== "linux" },
    () => {
      const binDir = mkdtempSync(join(tmpdir(), "fake-ldd-"));
      writeFileSync(
        join(binDir, "ldd"),
        "#!/bin/sh\necho 'musl libc (x86_64)'\n",
        {
          mode: 0o755,
        }
      );

      const result = spawnSync(process.execPath, [SCRIPT], {
        encoding: "utf8",
        env: { ...process.env, PATH: `${binDir}:${process.env.PATH ?? ""}` },
      });

      assert.equal(result.status, 0, result.stderr);
      assert.equal(result.stdout, `linux-${process.arch}-musl`);
    }
  );
});

describe("parseLibc", () => {
  it("detects musl", () => {
    assert.equal(parseLibc("musl libc (x86_64)\nVersion 1.2.5"), "musl");
  });

  it("defaults to gnu", () => {
    assert.equal(parseLibc("ldd (Ubuntu GLIBC 2.39-0ubuntu8.6) 2.39"), "gnu");
    assert.equal(parseLibc(""), "gnu");
  });
});
