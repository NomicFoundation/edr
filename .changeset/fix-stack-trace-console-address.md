---
"@nomicfoundation/edr": patch
---

Fixed Solidity stack-trace construction to recognise `console.log` calls by the Hardhat console address. It previously checked the `ecrecover` precompile address instead.
