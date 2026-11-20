---
area: rpc
surface: eth_estimateGas
---

# eth_estimateGas

Estimates the gas limit a transaction needs to succeed in the context of a block (default `pending`).

## eth_estimateGas.minimum-follows-block-hardfork

The intrinsic-gas minimum and the execution MUST use the hardfork of the target block, as defined by `hardfork-at-block.resolution`.

- **Applies to:** all chains; only observable on forks when estimating at a remote block whose hardfork differs from the configured one
- **Rationale:** a minimum computed under a different hardfork than the execution might over-estimate the gas
