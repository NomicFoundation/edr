---
"@nomicfoundation/edr": patch
---

Fixed `GASLIMIT` returning `1_000_000` while a block is mined with the block gas limit disabled. It now returns the parent block's gas limit, which the mined block's header already carried.
