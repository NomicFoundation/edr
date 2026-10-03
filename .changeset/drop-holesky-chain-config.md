---
"@nomicfoundation/edr": patch
---

Removed the built-in chain configuration of the Holesky testnet (chain ID 17000), which was shut down in 2025: EDR no longer ships its hardfork activation history and blob parameter schedule, nor a default RPC URL for the `holesky` chain alias in Solidity tests.
