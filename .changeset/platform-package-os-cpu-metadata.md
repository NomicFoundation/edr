---
"@nomicfoundation/edr": minor
---

Added the `os`, `cpu` and `libc` fields to the platform-specific `@nomicfoundation/edr-*` packages, so package managers install only the build matching the host. Without them, the `optionalDependencies` introduced in 0.16.0 still resolved to every platform.
