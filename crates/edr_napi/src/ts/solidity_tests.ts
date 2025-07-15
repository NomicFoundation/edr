import { StandardTestKind, FuzzTestKind, InvariantTestKind } from "../../index";

export enum SortOrder {
  Ascending,
  Descending,
}

export interface GasUsageFilter {
  minThreshold?: bigint;
  maxThreshold?: bigint;
}

function isStandardTestKind(object: any): object is StandardTestKind {
  return "consumedGas" in object;
}

function isFuzzTestKind(object: any): object is FuzzTestKind {
  return "runs" in object && "meanGas" in object && "medianGas" in object;
}

export function extractGasUsage(
  testResults: Array<{
    name: string;
    kind: StandardTestKind | FuzzTestKind | InvariantTestKind;
  }>,
  filter?: GasUsageFilter,
  ordering?: SortOrder
): Array<{ name: string; gas: bigint }> {
  const gasUsage: Array<{ name: string; gas: bigint }> = [];

  for (const result of testResults) {
    // Default to zero gas for invariant tests
    const gas = isStandardTestKind(result.kind)
      ? result.kind.consumedGas
      : isFuzzTestKind(result.kind)
        ? result.kind.medianGas
        : BigInt(0);

    if (
      (!filter?.minThreshold || gas >= filter.minThreshold) &&
      (!filter?.maxThreshold || gas <= filter.maxThreshold)
    ) {
      gasUsage.push({ name: result.name, gas });
    }
  }

  if (ordering === SortOrder.Ascending) {
    gasUsage.sort((a, b) => (a.gas < b.gas ? -1 : a.gas > b.gas ? 1 : 0));
  } else if (ordering === SortOrder.Descending) {
    gasUsage.sort((a, b) => (a.gas > b.gas ? -1 : a.gas < b.gas ? 1 : 0));
  }

  return gasUsage;
}
