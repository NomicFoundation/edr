---
"@nomicfoundation/edr": patch
---

Fixed the missing stack trace for an invariant test whose `afterInvariant()` reverts. The replay used to look for the revert reason in the passing `invariant()` call, so the runner discarded the failure's stack trace as unreproducible.
