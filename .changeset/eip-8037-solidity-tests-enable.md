---
"@nomicfoundation/edr": patch
---

Fixed EIP-8037 state gas accounting not being enabled in Solidity tests running on the Amsterdam hardfork, so `Vm.Gas.gasStateUsed` is now reported.
