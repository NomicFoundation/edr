import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  findTargetForPlatform,
  loadNapiConfig,
  napiHostConfig,
  type ParseTriple,
} from "./write_napi_host_config.ts";

const SCRIPTS_DIR = dirname(fileURLToPath(import.meta.url));
const SCRIPT = join(SCRIPTS_DIR, "write_napi_host_config.ts");
const NAPI_DIR = join(SCRIPTS_DIR, "..", "crates", "edr_napi");

// Loaded at module scope on purpose: a throw inside a `describe` body that has
// not registered a subtest yet is reported but still exits 0, so the suite
// below would silently stop being a gate.
const { parseTriple, targets } = loadNapiConfig(NAPI_DIR);

const platformSuffixes = readdirSync(join(NAPI_DIR, "npm"), {
  withFileTypes: true,
})
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name);

// Enough of @napi-rs/cli's mapping to exercise the lookup without resolving it.
const STUB_TRIPLES: Record<string, string> = {
  "aarch64-apple-darwin": "darwin-arm64",
  "x86_64-unknown-linux-gnu": "linux-x64-gnu",
  "x86_64-pc-windows-msvc": "win32-x64-msvc",
};

const workDir = mkdtempSync(join(tmpdir(), "napi-host-config-"));

after(() => {
  rmSync(workDir, { force: true, recursive: true });
});

const stubParseTriple: ParseTriple = (target) => {
  const platformArchABI = STUB_TRIPLES[target];

  if (platformArchABI === undefined) {
    throw new Error(`stub has no mapping for target ${target}`);
  }

  return { platformArchABI };
};

describe("findTargetForPlatform", () => {
  const stubTargets = Object.keys(STUB_TRIPLES);

  it("returns the target that builds the platform package", () => {
    assert.equal(
      findTargetForPlatform(stubTargets, "linux-x64-gnu", stubParseTriple),
      "x86_64-unknown-linux-gnu"
    );
  });

  it("returns undefined for a platform no target builds", () => {
    assert.equal(
      findTargetForPlatform(stubTargets, "linux-x64-musl", stubParseTriple),
      undefined
    );
  });

  it("returns undefined when no targets are configured", () => {
    assert.equal(
      findTargetForPlatform([], "linux-x64-gnu", stubParseTriple),
      undefined
    );
  });
});

describe("napiHostConfig", () => {
  it("narrows the config to the one target, newline-terminated", () => {
    assert.equal(
      napiHostConfig("x86_64-unknown-linux-gnu"),
      '{"targets":["x86_64-unknown-linux-gnu"]}\n'
    );
  });

  it("parses back as a NAPI-RS config", () => {
    assert.deepEqual(JSON.parse(napiHostConfig("aarch64-apple-darwin")), {
      targets: ["aarch64-apple-darwin"],
    });
  });
});

// The script's contract with edr_napi: every platform package the Verdaccio
// publish can detect must be buildable from a configured target, and vice
// versa. Uses the real parseTriple, so it also catches @napi-rs/cli changing
// the suffixes it derives.
describe("edr_napi platform packages", () => {
  it("has platform packages and targets configured", () => {
    assert.ok(platformSuffixes.length > 0, "no npm/ platform packages found");
    assert.ok(targets.length > 0, "napi.targets is empty");
  });

  for (const platformSuffix of platformSuffixes) {
    it(`has a napi target building ${platformSuffix}`, () => {
      assert.notEqual(
        findTargetForPlatform(targets, platformSuffix, parseTriple),
        undefined,
        `no napi target builds ${platformSuffix}; napi.targets: ${targets.join(", ")}`
      );
    });
  }

  for (const target of targets) {
    it(`has a platform package for ${target}`, () => {
      const { platformArchABI } = parseTriple(target);

      assert.ok(
        platformSuffixes.includes(platformArchABI),
        `napi target ${target} builds ${platformArchABI}, which has no npm/ directory; found: ${platformSuffixes.join(", ")}`
      );
    });
  }
});

// The emitted config only works because napi *merges* it over package.json's
// `napi` field. If an upgrade made it a replacement, binaryName would silently
// fall back to "index" and pre-publish would look for the wrong .node.
describe("emitted config", () => {
  it("is accepted by napi's own config reader", async () => {
    const configPath = join(workDir, "napi-merge.json");
    const napiRequire = createRequire(join(NAPI_DIR, "package.json"));
    const { readNapiConfig } = napiRequire("@napi-rs/cli");

    writeFileSync(configPath, napiHostConfig("x86_64-unknown-linux-gnu"));

    const config = await readNapiConfig(
      join(NAPI_DIR, "package.json"),
      configPath
    );

    assert.equal(config.binaryName, "edr");
    assert.deepEqual(
      config.targets.map(
        (target: { platformArchABI: string }) => target.platformArchABI
      ),
      ["linux-x64-gnu"]
    );
  });

  describe("written by the CLI", () => {
    it("writes the host config for a known platform", () => {
      const outPath = join(workDir, "known.json");
      const result = run(NAPI_DIR, "linux-x64-gnu", outPath);

      assert.equal(result.status, 0, result.stderr);
      assert.equal(
        readFileSync(outPath, "utf8"),
        '{"targets":["x86_64-unknown-linux-gnu"]}\n'
      );
    });

    it("exits 1 without writing for a platform no target builds", () => {
      const outPath = join(workDir, "unknown.json");
      const result = run(NAPI_DIR, "linux-riscv64-gnu", outPath);

      assert.equal(result.status, 1);
      assert.match(result.stderr, /no napi target builds/);
      assert.ok(!existsSync(outPath), "wrote a config despite failing");
    });

    it("exits 1 on missing arguments", () => {
      const result = run(NAPI_DIR, "linux-x64-gnu");

      assert.equal(result.status, 1);
      assert.match(result.stderr, /usage:/);
    });
  });
});

function run(...args: string[]) {
  return spawnSync(process.execPath, [SCRIPT, ...args], { encoding: "utf8" });
}
