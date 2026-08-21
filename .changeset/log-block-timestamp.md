---
"@nomicfoundation/edr": minor
---

Added the `blockTimestamp` field to logs, as specified by [ethereum/execution-apis#639](https://github.com/ethereum/execution-apis/pull/639).

Logs returned by `eth_getLogs`, `eth_getFilterLogs`, `eth_getFilterChanges` and in transaction receipts now carry `blockTimestamp`, the timestamp of the block the log is in, as a hex QUANTITY. Consumers indexing logs over a block range previously had to issue a second `eth_getBlockByHash` per block purely to timestamp them; this removes that round-trip. It matters most for browser-based indexers, where the provider often cannot batch those calls and each one costs a full round-trip.

The field is optional, matching the spec, and is populated for locally mined blocks. A log deserialized from a remote node that predates the spec change keeps it absent rather than defaulting it, so a missing timestamp stays distinguishable from a real one.

Closes #1643.
