---
"@nomicfoundation/edr": patch
---

Improved the performance of the `vm.addr`, `vm.sign` and `vm.signCompact` cheatcodes by caching the private key to address derivation per test suite.
