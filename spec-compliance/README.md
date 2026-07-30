# Spec Compliance

This directory documents EIPs and chain specifications that EDR does not support or only partially supports. Each entry links to a detailed document describing what the spec requires, how EDR deviates, and any available workarounds.

## Relation to Hardhat Network

Hardhat Network uses EDR as its execution substrate for local EVM simulation. Spec compliance gaps and EVM behavior issues documented here are owned by EDR; please open issues against this repository when the problem is about execution semantics or EIP/support status. Hardhat plugin and developer-experience issues (tasks, hooks, config, tooling) belong in [Hardhat](https://github.com/NomicFoundation/hardhat) instead.

## Status definitions

| Status | Meaning |
| --- | --- |
| **Unsupported** | EDR has no implementation related to this spec. |
| **Structural** | EDR includes the required fields defined by the spec to produce valid blocks, but does not simulate the underlying behavior. There are no plans to fully implement this. Workarounds may be available. |
| **Partial** | EDR has adapted tooling or provides workarounds, but the core logic is not yet implemented. |

## L1 Ethereum

| EIP | Hardfork | Status | Details |
| --- | --- | --- | --- |
| [EIP-4788](https://eips.ethereum.org/EIPS/eip-4788) | Cancun | Structural | [l1/eip-4788.md](l1/eip-4788.md) |

## OP Stack

| Spec | Hardfork | Status | Details |
| --- | --- | --- | --- |
| [`blob_gas_used` / DA Footprint](https://specs.optimism.io/protocol/jovian/exec-engine.html) | Jovian | Partial | [op/jovian_da_footprint.md](op/jovian_da_footprint.md) |
