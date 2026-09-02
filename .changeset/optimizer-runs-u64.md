---
"@nomicfoundation/edr": patch
---

Fixed build-info parsing rejecting compiler input whose `optimizer.runs` exceeds 32 bits. Projects using vanity runs values such as `444444444444` (aave-v4's historical setting) failed the whole build-info parse — disabling stack traces — even though solc accepts the value; the field is now read as 64-bit, matching solc and foundry-compilers.
