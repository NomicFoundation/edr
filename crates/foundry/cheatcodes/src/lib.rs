//! # foundry-cheatcodes
//!
//! Foundry cheatcodes implementations.

#![warn(
    unreachable_pub,
    unused_crate_dependencies,
    rust_2018_idioms
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![allow(elided_lifetimes_in_paths)] // Cheats context uses 3 lifetimes

#[macro_use]
pub extern crate foundry_cheatcodes_spec as spec;

#[macro_use]
extern crate tracing;

use alloy_primitives::Address;
pub use config::{CheatsConfig, CheatsConfigOptions, ExecutionContextConfig};
pub use endpoints::{RpcEndpoint, RpcEndpointUrl, RpcEndpoints};
pub use error::{Error, ErrorKind, Result};
use foundry_evm_core::{
    backend::CheatcodeBackend,
    evm_context::{EvmBuilderTrait, TransactionErrorTrait},
};
pub use fs_permissions::{FsAccessKind, FsAccessPermission, FsPermissions, PathPermission};
pub use inspector::{Cheatcodes, CheatcodesExecutor};
use revm::{
    context::{result::HaltReasonTr, CfgEnv},
    Journal,
};
use spec::Status;
pub use spec::{CheatcodeDef, Vm};

#[macro_use]
mod error;

mod base64;

mod cache;

mod config;

mod crypto;

mod endpoints;

mod env;

mod evm;

mod fs;

mod fs_permissions;

mod inspector;

mod json;

mod string;

mod test;

mod toml;

mod utils;

pub use cache::{CachedChains, CachedEndpoints, StorageCachingConfig};
use foundry_evm_core::evm_context::{BlockEnvTr, ChainContextTr, HardforkTr, TransactionEnvTr};
pub use test::expect::ExpectedCallTracker;
pub use Vm::ExecutionContext;

/// Cheatcode implementation.
pub(crate) trait Cheatcode: CheatcodeDef + DynCheatcode {
    /// Applies this cheatcode to the given state.
    ///
    /// Implement this function if you don't need access to the EVM data.
    #[allow(clippy::unimplemented)]
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let _ = state;
        unimplemented!("{}", Self::CHEATCODE.func.id)
    }

    /// Applies this cheatcode to the given context.
    ///
    /// Implement this function if you need access to the EVM data.
    #[inline(always)]
    fn apply_stateful<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<'_, '_, BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT, DatabaseT>
    ) -> Result {
        self.apply::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT, DatabaseT>(ccx.state)
    }

    /// Applies this cheatcode to the given context and executor.
    ///
    /// Implement this function if you need access to the executor.
    #[inline(always)]
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
        executor: &mut dyn CheatcodesExecutor<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >
    ) -> Result {
        let _ = executor;
        self.apply_stateful(ccx)
    }
}
pub(crate) trait DynCheatcode: IsPure {
    fn cheatcode(&self) -> &'static foundry_cheatcodes_spec::Cheatcode<'static>;
    fn name(&self) -> &'static str;
    fn id(&self) -> &'static str;
    fn signature(&self) -> &'static str;
    fn status(&self) -> &Status<'static>;
    fn as_debug(&self) -> &dyn std::fmt::Debug;
}

impl<
    T: Cheatcode> DynCheatcode for T {
    #[inline]
    fn cheatcode(&self) -> &'static foundry_cheatcodes_spec::Cheatcode<'static> {
        T::CHEATCODE
    }

    fn name(&self) -> &'static str {
        self.cheatcode().func.signature.split('(').next().unwrap()
    }

    fn id(&self) -> &'static str {
        self.cheatcode().func.id
    }

    fn signature(&self) -> &'static str {
        self.cheatcode().func.signature
    }

    fn status(&self) -> &Status<'static> {
        &self.cheatcode().status
    }

    #[inline]
    fn as_debug(&self) -> &dyn std::fmt::Debug {
        self
    }
}

/// The cheatcode context, used in [`Cheatcode`].
pub(crate) struct CheatsCtxt<
    'cheats,
    'evm,
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
    DatabaseT: CheatcodeBackend<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
    >,
> {
    /// The cheatcodes inspector state.
    pub(crate) state: &'cheats mut Cheatcodes<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    /// The EVM data.
    pub(crate) ecx: &'evm mut revm::context::Context<
        BlockT,
        TxT,
        CfgEnv<HardforkT>,
        DatabaseT,
        Journal<DatabaseT>,
        ChainContextT,
    >,
    /// The original `msg.sender`.
    pub(crate) caller: Address,
}

// TODO remove this
impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    > std::ops::Deref
    for CheatsCtxt<
        '_,
        '_,
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    >
{
    type Target =
        revm::context::Context<BlockT, TxT, CfgEnv<HardforkT>, DatabaseT, Journal<DatabaseT>, ChainContextT>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.ecx
    }
}

// TODO remove this
impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    > std::ops::DerefMut
    for CheatsCtxt<
        '_,
        '_,
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    >
{
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.ecx
    }
}

impl<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >
    CheatsCtxt<
        '_,
        '_,
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    >
{
    #[inline]
    pub(crate) fn is_precompile(&self, address: &Address) -> bool {
        self.ecx.journaled_state.inner.precompiles.contains(address)
    }
}

pub(crate) trait IsPure {
    /// Whether the cheatcode is a pure function if its inputs.
    /// If it's not, that means it's not safe to re-execute a call that invokes
    /// it and expect the same results.
    fn is_pure(&self) -> bool;
}

/// Implement `IsPure::is_pure` to return `true`.
#[macro_export]
macro_rules! impl_is_pure_true {
    ($type:ty) => {
        impl $crate::IsPure for $type {
            #[inline(always)]
            fn is_pure(&self) -> bool {
                true
            }
        }
    };
}

/// Implement `IsPure::is_pure` to return `false`.
#[macro_export]
macro_rules! impl_is_pure_false {
    ($type:ty) => {
        impl $crate::IsPure for $type {
            #[inline(always)]
            fn is_pure(&self) -> bool {
                false
            }
        }
    };
}
