---
"@nomicfoundation/edr": patch
---

Fixed stack traces for contracts compiled with the slang compiler reporting a revert at the function declaration instead of invalid arguments when the calldata cannot be decoded, e.g. when it is truncated.
