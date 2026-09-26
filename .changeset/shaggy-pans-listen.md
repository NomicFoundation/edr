---
"@nomicfoundation/edr": patch
---

Fixed a leak that kept a provider alive after the consumer dropped their last reference to it, along with the OS thread the provider owns. Every callback a provider is given used to keep it alive when the callback reached back to the provider, directly or through a wrapper object. The affected callbacks are the subscription callback, the logger's `printLine` and `decodeConsoleLogInputs` (even with `enable: false`), a call override, and the coverage and gas-report callbacks. Callbacks may now capture their provider freely, so reaching it through a `WeakRef` is no longer necessary.

Each provider now owns its callbacks under a non-enumerable, non-writable, non-configurable `__edrCallbacks` property on its own object. `Object.getOwnPropertyNames(provider)` gains that one entry, while `Object.keys` and `JSON.stringify` are unaffected. A request still in flight when the consumer drops its last provider reference may now fail, because the provider's callbacks no longer outlive their owner.
