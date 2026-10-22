---
"@nomicfoundation/edr": patch
---

Fixed Solidity test stack traces for cheatcode errors (e.g. a failed `expectRevert`) pointing to a different test function when the optimizer deduplicates code between them.
