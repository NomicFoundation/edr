---
"@nomicfoundation/edr": minor
---

Changed the platform-specific `@nomicfoundation/edr-*` packages from `dependencies` to `optionalDependencies`, so installs only download the build for the current platform instead of all of them. Note: npm < 11.3.0 may skip the platform package when reusing a lockfile created on a different platform (npm/cli#4828); if affected, upgrade with `npm install -g npm@11`.
