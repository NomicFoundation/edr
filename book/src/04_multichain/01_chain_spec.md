# Chain Specification

EDR uses a concept called _chain specification_ to define all necessary types and functionality to support an Ethereum chain at compile-time.

This is achieved through the usage of multiple traits, some of which are supertraits of each other, providing increasing scope of functionality in the EDR ecosystem.
All traits follow the same naming pattern: `*ChainSpec`; e.g. `EvmChainSpec` and `BlockChainSpec`.

Most of these traits have a `Sync*` equivalent (e.g. `SyncBlockChainSpec`) which are automatically implemented for usage in `async` contexts.

Crates including the chain specification traits are located in the `crates/chain/spec` folder.

## Supported Chain Types

Currently, EDR supports the following chain types out-of-the-box.

- Generic
- L1 Ethereum
- OP

## Adding your own Chain Type

To add a new chain type, add a new unit struct. E.g.

```rs
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct GenericChainSpec;
```

Depending on which functionality you want to support for your chain type, you can implement some or all of the traits outlined [above](#chain-specification).
