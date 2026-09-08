---
"@nomicfoundation/edr": patch
---

Fixed a leak that kept a provider, and the OS thread it owns, alive after the consumer dropped their last reference to it. Every callback a provider is given — the subscription callback, the logger's `printLine` and `decodeConsoleLogInputs` (even with `enable: false`), a call override, and the coverage and gas-report callbacks — used to keep its provider alive if it reached back to it, directly or through a wrapper object. Callbacks may now capture their provider freely, so reaching it through a `WeakRef` is no longer necessary.

Each provider now owns its callbacks under a non-enumerable `__edrCallbacks` property on its own object, so code that enumerates a provider's own property names sees one extra entry.
