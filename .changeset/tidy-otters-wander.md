---
"@nomicfoundation/edr": minor
---

Fixed interval mining accepting a `[min, max]` range that it could not honour. A range with `min > max` crashed the provider on its first interval-mined block, and `[0, 0]` starved the provider of incoming requests. A range must now satisfy `1 <= min <= max`, and is rejected when the provider is created, when `evm_setIntervalMining` is called (JSON-RPC error `-32602`), and when a recorded scenario is loaded. Both bounds remain inclusive.

Fixed `evm_setIntervalMining` not restarting the interval timer when called with the interval that was already configured. A range draws a new interval before each block, so an unchanged configuration does not imply an unchanged schedule.

BREAKING CHANGE: `[0, 0]` and `[0, N]` interval ranges are no longer accepted. Neither previously worked as configured, but a configuration that is forwarded today will now fail when the provider is created. A scalar `0` still disables interval mining; a zero minimum inside a range must be normalised by the caller.
