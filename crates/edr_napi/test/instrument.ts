import { assert, expect } from "chai";
import * as fs from "fs";

import {
  addStatementCoverageInstrumentation,
  InstrumentationMetadata,
} from "..";
import { toBuffer } from "./helpers";

function assertMetadata(
  metadata: InstrumentationMetadata,
  expected: InstrumentationMetadata
) {
  assert.deepEqual(metadata.tag, expected.tag);
  assert.strictEqual(metadata.kind, expected.kind);
  assert.strictEqual(metadata.startUtf16, expected.startUtf16);
  assert.strictEqual(metadata.endUtf16, expected.endUtf16);
}

function readSourceCode(): string {
  const filePath = `${__dirname}/../../../data/contracts/instrumentation.sol`;
  return fs.readFileSync(filePath, "utf8");
}

describe("Code coverage", () => {
  const incrementSourceCode = readSourceCode();

  describe("instrumentation", function () {
    it("Statement coverage", async function () {
      const result = addStatementCoverageInstrumentation(
        incrementSourceCode,
        "instrumentation.sol",
        "0.8.0"
      );

      expect(result.source).to.contain("__HardhatCoverage.sendHit(");

      assert.lengthOf(result.metadata, 3);
      assertMetadata(result.metadata[0], {
        tag: toBuffer(
          "0xdaa9804f41c839f316b418296d7b0ad8d91ca024d803ab632e9fd32d896f429b"
        ),
        kind: "statement",
        startUtf16: 43,
        endUtf16: 59,
      });

      assertMetadata(result.metadata[1], {
        tag: toBuffer(
          "0x4b739f4956f43f9e2e753cecfe2569672686cba78a199684075dc494bc60b06b"
        ),
        kind: "statement",
        startUtf16: 59,
        endUtf16: 75,
      });

      assertMetadata(result.metadata[2], {
        tag: toBuffer(
          "0x9f4fc9ded31350bade85ee54fc2d6dd8d0609fbe0f42203ab07c9a32b95fa4c4"
        ),
        kind: "statement",
        startUtf16: 75,
        endUtf16: 95,
      });
    });
  });
});
