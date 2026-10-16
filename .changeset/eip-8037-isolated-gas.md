---
"@nomicfoundation/edr": patch
---

Fixed isolated-mode gas accounting: isolated calls and creates now keep their EIP-8037 state gas reservoir and real intrinsic gas, `Vm.Gas.gasTotalUsed` reports the transaction's regular gas before refunds, gas snapshots report the transaction's receipt gas, frames whose intrinsic gas or calldata floor exceeds the forwarded gas consume their whole budget instead of reporting zero, and isolated calls below the EIP-7623 calldata floor no longer revert. Consecutive `startSnapshotGas` regions in one function no longer include the previous region's cheatcode overhead in their first sample.
