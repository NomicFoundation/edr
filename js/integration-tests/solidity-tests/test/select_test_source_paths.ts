import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { selectTestSourcePaths } from "@nomicfoundation/edr-helpers";

describe("selectTestSourcePaths", () => {
  it("maps every source when all suites are 0.8.0 or newer", () => {
    const { testSourcePaths, unparsableSources } = selectTestSourcePaths([
      {
        source: "project/A.sol",
        userSourceName: "A.sol",
        path: "/project/A.sol",
        solcVersion: "0.8.0",
      },
      {
        source: "project/B.sol",
        userSourceName: "B.sol",
        path: "/project/B.sol",
        solcVersion: "0.8.30",
      },
    ]);

    assert.deepEqual(testSourcePaths, {
      "project/A.sol": "/project/A.sol",
      "project/B.sol": "/project/B.sol",
    });
    assert.deepEqual(unparsableSources, []);
  });

  it("empties the map and names the user-facing source when one suite predates 0.8.0", () => {
    const { testSourcePaths, unparsableSources } = selectTestSourcePaths([
      {
        source: "project/A.sol",
        userSourceName: "A.sol",
        path: "/project/A.sol",
        solcVersion: "0.8.0",
      },
      {
        source: "project/test-contracts/Old.sol",
        userSourceName: "test-contracts/Old.sol",
        path: "/project/test-contracts/Old.sol",
        solcVersion: "0.7.6",
      },
    ]);

    assert.deepEqual(testSourcePaths, {});
    assert.deepEqual(unparsableSources, ["test-contracts/Old.sol"]);
  });

  it("deduplicates and sorts the unparsable source names", () => {
    const { testSourcePaths, unparsableSources } = selectTestSourcePaths([
      {
        source: "project/B.sol",
        userSourceName: "B.sol",
        path: "/project/B.sol",
        solcVersion: "0.7.6",
      },
      {
        source: "project/A.sol",
        userSourceName: "A.sol",
        path: "/project/A.sol",
        solcVersion: "0.7.6",
      },
      {
        source: "project/B.sol",
        userSourceName: "B.sol",
        path: "/project/B2.sol",
        solcVersion: "0.6.12",
      },
    ]);

    assert.deepEqual(testSourcePaths, {});
    assert.deepEqual(unparsableSources, ["A.sol", "B.sol"]);
  });
});
