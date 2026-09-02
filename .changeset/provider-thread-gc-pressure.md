---
"@nomicfoundation/edr": patch
---

Fixed provider threads accumulating until the process ran out of them. Each provider owns a dedicated OS thread, which V8 could not see, so an unreachable provider was never collected and its thread never joined. On macOS, whose per-process thread limit is far below Linux's, a suite creating hundreds of providers exhausted it.

`createProvider` now rejects when the OS refuses a thread, instead of panicking and leaving its promise pending forever.
