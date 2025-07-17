"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.SortOrder = void 0;
exports.extractGasUsage = extractGasUsage;
var SortOrder;
(function (SortOrder) {
    SortOrder[SortOrder["Ascending"] = 0] = "Ascending";
    SortOrder[SortOrder["Descending"] = 1] = "Descending";
})(SortOrder || (exports.SortOrder = SortOrder = {}));
function isStandardTestKind(object) {
    return "consumedGas" in object;
}
function isFuzzTestKind(object) {
    return "runs" in object && "meanGas" in object && "medianGas" in object;
}
function extractGasUsage(testResults, filter, ordering) {
    const gasUsage = [];
    for (const result of testResults) {
        // Default to zero gas for invariant tests
        const gas = isStandardTestKind(result.kind)
            ? result.kind.consumedGas
            : isFuzzTestKind(result.kind)
                ? result.kind.medianGas
                : BigInt(0);
        if ((!filter?.minThreshold || gas >= filter.minThreshold) &&
            (!filter?.maxThreshold || gas <= filter.maxThreshold)) {
            gasUsage.push({ name: result.name, gas });
        }
    }
    if (ordering === SortOrder.Ascending) {
        gasUsage.sort((a, b) => (a.gas < b.gas ? -1 : a.gas > b.gas ? 1 : 0));
    }
    else if (ordering === SortOrder.Descending) {
        gasUsage.sort((a, b) => (a.gas > b.gas ? -1 : a.gas < b.gas ? 1 : 0));
    }
    return gasUsage;
}
//# sourceMappingURL=solidity_tests.js.map