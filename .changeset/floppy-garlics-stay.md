---
"@nomicfoundation/edr": patch
---

Added experimental EIP-7928 support: blocks on Amsterdam+ now include the `blockAccessListHash` header field. The value is simulated, not the real `keccak256(rlp(blockAccessList))`. Within a single blockchain it is unique per block, and a block with no state changes uses the empty-list hash `keccak256(rlp([]))` as the EIP specifies; it is not, however, guaranteed to be consistent across provider configurations (e.g. a different hardfork).
