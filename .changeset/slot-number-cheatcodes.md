---
"@nomicfoundation/edr": patch
---

- Added the `rollSlot(uint64)` and `getSlotNumber()` cheatcodes, which set and read `block.slotnum` (EIP-7843) without changing the block number or timestamp. Both revert on EVM versions before Amsterdam.
- Fixed `block.slotnum` returning 0 in forked Solidity tests instead of the forked block's slot number.
