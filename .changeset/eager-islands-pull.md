---
"@nomicfoundation/edr": patch
---

Added experimental EIP-8037 support: from the Amsterdam hardfork, blocks meter execution gas and state gas separately, `gasUsed` reports the larger of the two, transactions are admitted per dimension, the EIP-7825 gas cap applies to execution gas only, and `tx.gas` is capped at 2^32 − 1. Transaction receipts are unchanged.
