---
"@nomicfoundation/edr": minor
---

- Fixed EIP-8037 state gas accounting not being enabled in Solidity tests running on the Amsterdam hardfork, including isolated calls and creates, so `Vm.Gas.gasStateUsed` is now reported. `Vm.Gas.gasTotalUsed` excludes state gas that spilled past the reservoir.
- Fixed isolated Solidity test calls and creates paying a flat 21000 intrinsic gas instead of the transaction's real intrinsic gas, which ignored calldata, access lists, authorizations and init code on every hardfork.
- Changed `Vm.Gas.gasTotalUsed` for isolated frames to report the transaction's regular gas before refunds, with `gasRefunded` reported separately.
- Changed `snapshotGasLastCall`, `snapshotGasLastFrame` and `startSnapshotGas`/`stopSnapshotGas` to report the receipt gas of an isolated frame's transaction, so the value no longer depends on the gas forwarded to the call or on refunds.
- Fixed isolated frames whose intrinsic gas or calldata floor exceeded the forwarded gas reporting zero gas and returning the forwarded gas to the caller. They now consume their whole budget.
- Fixed isolated calls reverting when the gas forwarded to them is below their calldata's EIP-7623 floor, while the same call succeeds without isolation.
- Fixed consecutive `startSnapshotGas` regions in one function including the previous region's cheatcode overhead in their value.
- Changed isolated frames to no longer be validated against the transaction gas cap and the block gas limit, which the enclosing test transaction already satisfied.
