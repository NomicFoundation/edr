---
"@nomicfoundation/edr": patch
---

Added experimental EIP-7928 support: blocks on Amsterdam+ now include the `blockAccessListHash` header field. The value is simulated, not the real `keccak256(rlp(blockAccessList))`.
