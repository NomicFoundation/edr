---
"@nomicfoundation/edr": minor
---

Raised the minimum glibc of the prebuilt Linux gnu binaries (`@nomicfoundation/edr-linux-x64-gnu`, `@nomicfoundation/edr-linux-arm64-gnu`) from 2.30 to 2.34. They are now built on Debian 12 (bookworm) instead of Debian 11 (bullseye), which reached end of life on 2026-08-31.

Ubuntu 20.04 and Debian 11 can no longer load these binaries and will fail at require time with `GLIBC_2.34 not found`. Ubuntu 22.04, Debian 12, RHEL 9, Amazon Linux 2023 and later are unaffected, as are the musl builds.
