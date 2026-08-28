---
"@nomicfoundation/edr": minor
---

Interval mining now validates its configuration up front. A `[min, max]` interval range must satisfy `1 <= min <= max`, and is rejected when the provider is created, when `evm_setIntervalMining` is called (JSON-RPC error `-32602`), and when a recorded scenario is loaded.

This is a breaking change: `[0, 0]` and `[0, N]` were previously accepted. `[0, 0]` starved the provider of incoming requests, and a range with `min > max` crashed the provider on its first interval-mined block, so neither previously worked as configured — but a configuration that Hardhat forwards today will now fail at provider creation. Hardhat converts a scalar `0` to "disabled" before reaching EDR; it does not yet normalise a zero minimum inside a range, which it must do to guarantee forwards compatibility.

Both bounds remain inclusive.

`evm_setIntervalMining` now restarts the interval timer on every call, including when it sets the interval that is already configured, matching Hardhat.
