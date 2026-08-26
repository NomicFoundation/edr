---
"@nomicfoundation/edr": minor
---

Added a version subdirectory to the RPC response cache on disk, to invalidate outdated cache types. Cache entries now live under `rpc_cache/v2`, so everything else in `rpc_cache` is ignored and can be deleted.
