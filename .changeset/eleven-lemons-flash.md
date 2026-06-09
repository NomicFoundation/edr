---
"@nomicfoundation/edr": patch
---

Fixed `eth_estimateGas` erroring on Osaka+ when the binary search probed gas values above the EIP-7825 per-transaction cap
