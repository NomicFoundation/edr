---
"@nomicfoundation/edr": minor
---

Added the `lastFrameGas` and `snapshotGasLastFrame` cheatcodes, which also record CREATE and CREATE2 frames, and deprecated `lastCallGas` and `snapshotGasLastCall` in their favour. The `Vm.Gas` struct gained a sixth field, `int64 gasStateUsed`;the field is always zero for now, as EIP-8037 state gas is not supported yet.
