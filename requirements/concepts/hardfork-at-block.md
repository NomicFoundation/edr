---
area: concepts
---

# Hardfork at a block

The hardfork EDR executes a transaction under at a given block. On a fork, blocks at or before the fork block are remote blocks with their own history; later blocks are mined locally under the configured hardfork. Remote blocks use the remote chain's activation schedule, which chain overrides can change.

## hardfork-at-block.resolution

The hardfork a transaction executes under at a block given by tag, number or hash MUST be:

| Block | Hardfork |
| --- | --- |
| `pending` | the configured hardfork |
| locally mined block (local chain, or after the fork block) | the configured hardfork |
| remote block (at or before the fork block), including `latest` on a fork with no locally mined blocks | the hardfork active at that block's number and timestamp in the remote chain's activation schedule |

- **Applies to:** all chains

## hardfork-at-block.unknown-remote-chain

A request that needs the hardfork of a remote block MUST fail with an error when EDR knows no hardfork for that block, instead of assuming one.

- **Applies to:** forked chains whose activation schedule is empty, or blocks older than its first activation
