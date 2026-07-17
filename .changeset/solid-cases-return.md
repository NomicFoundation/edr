---
"@nomicfoundation/edr": patch
---

Added experimental EIP-7843 support: blocks on Amsterdam+ now include the `slotNumber` header field, and the `SLOTNUM` (`0x4b`) opcode returns it. EDR has no consensus layer, so the value is simulated: increments by one per mined block, starting at 0 on local blockchains.
