---
"@nomicfoundation/edr": minor
---

Added an optional `blockTimestamp` field to logs, as specified by [ethereum/execution-apis#639](https://github.com/ethereum/execution-apis/pull/639). Logs returned by `eth_getLogs`, `eth_getFilterLogs`, `eth_getFilterChanges`, `eth_subscribe("logs")` and in transaction receipts now carry the timestamp of the block the log is in, as a hex QUANTITY.

Matching the spec, the field is populated for locally mined blocks. When forking, it is passed through from the remote node, which every major client now serves; a log from a node that predates the spec change keeps the field absent rather than defaulting it, so a missing timestamp stays distinguishable from a real one.
