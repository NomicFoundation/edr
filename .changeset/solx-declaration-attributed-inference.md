---
"@nomicfoundation/edr": patch
---

Fixed solx stack traces misreporting returndata-size mismatches and calls to codeless accounts as generic reverts at the call site, dropping the called function's frame from cross-contract modifier reverts, and — under non-default optimizer modes — reporting modifier reverts at the function declaration line or collapsing bare modifier reverts to `OtherExecutionError` at the contract declaration.
