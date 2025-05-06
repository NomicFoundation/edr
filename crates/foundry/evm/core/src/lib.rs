//! # foundry-evm-core
//!
//! Core EVM abstractions.

#![warn(unused_crate_dependencies)]

use auto_impl::auto_impl;
use revm::{
    context::{Block, CfgEnv},
    context_interface::Transaction,
    inspector::NoOpInspector,
    primitives::hardfork::SpecId,
    Context, Database, Inspector, Journal,
};
use revm_inspectors::access_list::AccessListInspector;

#[macro_use]
extern crate tracing;

mod ic;

pub mod abi;
pub mod backend;
pub mod constants;
pub mod contracts;
pub mod decode;
pub mod evm_env;
pub mod fork;
pub mod opcodes;
pub mod opts;
pub mod precompiles;
pub mod snapshot;
pub mod utils;

/// An extension trait that allows us to add additional hooks to Inspector for
/// later use in handlers.
#[auto_impl(&mut, Box)]
pub trait InspectorExt<BlockT, TxT, HardforkT, DatabaseT, ChainContextT>:
    Inspector<Context<BlockT, TxT, CfgEnv<HardforkT>, DatabaseT, Journal<DatabaseT>, ChainContextT>>
where
    BlockT: Block,
    TxT: Transaction,
    HardforkT: Into<SpecId> + Copy,
    DatabaseT: Database,
{
    // Simulates `console.log` invocation.
    fn console_log(&mut self, _input: String) {}
}

impl<BlockT, TxT, HardforkT, DatabaseT, ChainContextT>
    InspectorExt<BlockT, TxT, HardforkT, DatabaseT, ChainContextT> for NoOpInspector
where
    BlockT: Block,
    TxT: Transaction,
    HardforkT: Into<SpecId> + Copy,
    DatabaseT: Database,
{
}

impl<BlockT, TxT, HardforkT, DatabaseT, ChainContextT>
    InspectorExt<BlockT, TxT, HardforkT, DatabaseT, ChainContextT> for AccessListInspector
where
    BlockT: Block,
    TxT: Transaction,
    HardforkT: Into<SpecId> + Copy,
    DatabaseT: Database,
{
}
