import { StandardTestKind, FuzzTestKind, InvariantTestKind } from "../../index";
export declare enum SortOrder {
    Ascending = 0,
    Descending = 1
}
export interface GasUsageFilter {
    minThreshold?: bigint;
    maxThreshold?: bigint;
}
export declare function extractGasUsage(testResults: Array<{
    name: string;
    kind: StandardTestKind | FuzzTestKind | InvariantTestKind;
}>, filter?: GasUsageFilter, ordering?: SortOrder): Array<{
    name: string;
    gas: bigint;
}>;
//# sourceMappingURL=solidity_tests.d.ts.map