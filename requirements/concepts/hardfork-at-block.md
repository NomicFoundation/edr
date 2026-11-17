---
area: concepts
---

# Hardfork at a block

Methods that take a block tag or number run under the hardfork of that block. On a fork, blocks at or before the fork block are remote blocks with their own history; later blocks are mined locally under the configured hardfork.

## hardfork-at-block.resolution

The hardfork for a block tag or number MUST be:

| Block | Hardfork |
| --- | --- |
| `pending` | the configured hardfork |
| locally mined block (local chain, or after the fork block) | the configured hardfork |
| remote block (at or before the fork block) | the remote chain's hardfork active at that block's number and timestamp |

`latest` on a fork with no locally mined blocks resolves to the fork block, so it is a remote block.

- **Applies to:** all methods that accept a block tag or number

## hardfork-at-block.unknown-remote-chain

A request for a remote block MUST fail with an error when EDR has no hardfork activations for the remote chain, instead of assuming a hardfork.

- **Applies to:** forked chains without built-in activations or a chain override
