---
"@nomicfoundation/edr": minor
---

Fixed coverage instrumentation calls clearing the EVM's returndata buffer, breaking RETURNDATASIZE and RETURNDATACOPY opcodes in instrumented contracts.
