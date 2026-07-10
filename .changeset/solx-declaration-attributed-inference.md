---
"@nomicfoundation/edr": patch
---

Fixed solx stack traces degrading to `OtherExecutionError` at the contract declaration for returndata-size mismatches and calls to codeless accounts, and reporting modifier reverts at the function declaration line instead of the failing statement.
