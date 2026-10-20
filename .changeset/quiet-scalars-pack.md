---
"@nomicfoundation/edr": patch
---

Fixed simulated OP networks not charging the L1 data fee: the `L1Block` predeploy's genesis storage now sets `baseFeeScalar` and `blobBaseFeeScalar`, so transactions pay the fee and receipts report it.
