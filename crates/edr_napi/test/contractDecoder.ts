import { assert } from "chai";
import * as path from "path";
import * as fs from "fs";
import * as os from "os";

import { ContractDecoder } from "..";

describe("ContractDecoder", () => {
  describe("fromProject", () => {
    // A real-world Hardhat v3 artifacts directory with build infos.
    const artifactsDir = path.join(
      __dirname,
      "../../../js/integration-tests/solidity-tests/artifacts"
    );

    it("loads build infos from an artifacts directory", function () {
      if (!fs.existsSync(path.join(artifactsDir, "build-info"))) {
        this.skip();
      }

      const decoder = ContractDecoder.fromProject({ artifactsDir });
      assert.instanceOf(decoder, ContractDecoder);
    });

    it("accepts an explicit build info directory", function () {
      const buildInfoDir = path.join(artifactsDir, "build-info");
      if (!fs.existsSync(buildInfoDir)) {
        this.skip();
      }

      const decoder = ContractDecoder.fromProject({
        artifactsDir,
        buildInfoDir,
        ignoreContracts: false,
      });
      assert.instanceOf(decoder, ContractDecoder);
    });

    it("creates an empty decoder for a project without build infos", function () {
      const decoder = ContractDecoder.fromProject({
        artifactsDir: path.join(__dirname, "does-not-exist"),
      });
      assert.instanceOf(decoder, ContractDecoder);
    });

    it("throws on invalid build info files", function () {
      const brokenDir = fs.mkdtempSync(
        path.join(os.tmpdir(), "edr-broken-build-info-")
      );
      try {
        const buildInfoDir = path.join(brokenDir, "build-info");
        fs.mkdirSync(buildInfoDir);
        fs.writeFileSync(path.join(buildInfoDir, "solc-0_8_24-aa.json"), "");
        fs.writeFileSync(
          path.join(buildInfoDir, "solc-0_8_24-aa.output.json"),
          ""
        );

        assert.throws(() => {
          ContractDecoder.fromProject({ artifactsDir: brokenDir });
        }, /Failed to parse build info/);
      } finally {
        fs.rmSync(brokenDir, { recursive: true, force: true });
      }
    });
  });
});
