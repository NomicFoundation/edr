import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  StandardTestKind,
  FuzzTestKind,
  InvariantTestKind,
} from "@nomicfoundation/edr";

import {
  extractGasUsage,
  GasUsageFilter,
  SortOrder,
} from "@nomicfoundation/edr/solidity-tests";

describe("Gas tests", () => {
  let testResults: Array<{
    name: string;
    kind: StandardTestKind | FuzzTestKind | InvariantTestKind;
  }> = [
    {
      name: "Standard Test",
      kind: {
        consumedGas: BigInt(1000),
      },
    },
    {
      name: "Fuzz Test",
      kind: {
        runs: BigInt(10),
        meanGas: BigInt(2000),
        medianGas: BigInt(1500),
      },
    },
    {
      name: "Invariant Test",
      kind: {
        runs: BigInt(5),
        calls: BigInt(100),
        reverts: BigInt(0),
      },
    },
  ];

  it("basic extractGasUsage", async function () {
    const simpleGasUsage = extractGasUsage(testResults);
    assert.equal(simpleGasUsage.length, 3);
    assert.deepEqual(simpleGasUsage[0], {
      name: "Standard Test",
      gas: BigInt(1000),
    });
    assert.deepEqual(simpleGasUsage[1], {
      name: "Fuzz Test",
      gas: BigInt(1500), // medianGas for Fuzz Test
    });
    assert.deepEqual(simpleGasUsage[2], {
      name: "Invariant Test",
      gas: BigInt(0), // Default for Invariant Test
    });
  });

  it("filtered extractGasUsage", async function () {
    const filter: GasUsageFilter = {
      minThreshold: BigInt(1),
      maxThreshold: BigInt(1000),
    };
    const filteredGasUsage = extractGasUsage(testResults, filter);

    assert.equal(filteredGasUsage.length, 1);
    assert.deepEqual(filteredGasUsage[0], {
      name: "Standard Test",
      gas: BigInt(1000),
    });
  });

  it("sorted extractGasUsage", async function () {
    const ascendingGasUsage = extractGasUsage(
      testResults,
      undefined,
      SortOrder.Ascending
    );

    assert.equal(ascendingGasUsage.length, 3);
    assert.deepEqual(ascendingGasUsage[0], {
      name: "Invariant Test",
      gas: BigInt(0), // Default for Invariant Test
    });
    assert.deepEqual(ascendingGasUsage[1], {
      name: "Standard Test",
      gas: BigInt(1000),
    });
    assert.deepEqual(ascendingGasUsage[2], {
      name: "Fuzz Test",
      gas: BigInt(1500), // medianGas for Fuzz Test
    });
  });

  it("filtered and sorted extractGasUsage", async function () {
    const filter: GasUsageFilter = {
      minThreshold: BigInt(1), // exclude Invariant Test
    };
    const gasUsage = extractGasUsage(testResults, filter, SortOrder.Descending);

    assert.equal(gasUsage.length, 2);
    assert.deepEqual(gasUsage[0], {
      name: "Fuzz Test",
      gas: BigInt(1500), // medianGas for Fuzz Test
    });
    assert.deepEqual(gasUsage[1], {
      name: "Standard Test",
      gas: BigInt(1000),
    });
  });
});
