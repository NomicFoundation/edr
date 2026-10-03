---
"@nomicfoundation/edr": patch
---

Fixed the transaction `Value` shown in the node logs dropping the leading zeros of the fractional part (e.g. 0.01 ETH was logged as `0.1 ETH`), and whole amounts being logged with a trailing dot (e.g. `1. ETH`).
