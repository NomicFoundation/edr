# Jovian: `blob_gas_used` / DA Footprint

**Spec**: <https://specs.optimism.io/protocol/jovian/exec-engine.html> **Hardfork**: Jovian **Status**: Partial

## Terminology

The spec, JSON-RPC, and EDR internals use different names for the field affected by this spec gap:

| Spec / JSON-RPC | EDR internal (`BlobGas` struct) |
| --------------- | ------------------------------- |
| `blobGasUsed`   | `blob_gas.gas_used`             |

This document uses `blobGasUsed` (the JSON-RPC / spec name) throughout.

## What the spec requires

Starting with the Jovian hardfork, the OP Stack repurposes `blobGasUsed` to store the block's DA (Data Availability) footprint. The DA footprint is computed from the estimated compressed size of each non-deposit transaction:

```
daUsageEstimate = max(
    minTransactionSize,
    (intercept + fastlzCoef * tx.fastlzSize) // 1e6
)

daFootprint += daUsageEstimate * daFootprintGasScalar
```

The block's `blobGasUsed` is then set to its total `daFootprint`. This value also affects base fee calculations: `gasMetered = max(gasUsed, blobGasUsed)` replaces the previous `gasUsed` in the EIP-1559 base fee formula.

## What EDR does

EDR does not currently compute `blobGasUsed` using the `daFootprint` calculation. The DA footprint estimation (FastLZ compression, scalar parameters) is not implemented.

## Impact

- `blobGasUsed` in block headers will not reflect the DA footprint as specified
- Base fee calculations for subsequent blocks may diverge from the spec, since `gasMetered` depends on `blobGasUsed`

## Workarounds

The block replay tool works around this by overriding the `blob_gas` header field with the remote block's value via header overrides (see `jovian_header_overrides` in `crates/edr_op/src/test_utils.rs`). This ensures replayed blocks have the correct `blobGasUsed` without EDR needing to compute the DA footprint locally.

The same approach can be used at the provider level: override the `blob_gas` header field with the expected value when mining blocks.

## Related issues

- [#1212 — Fully support OP Jovian hardfork](https://github.com/NomicFoundation/edr/issues/1212): implementing the DA footprint calculation would change this status from Partial to fully supported.
