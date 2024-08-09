use core::fmt::Debug;

use edr_eth::{
    chain_spec::L1ChainSpec,
    transaction::signed::{FakeSign, Sign},
    Address, BlockSpec, Bytes, SpecId, U256,
};
use edr_evm::{
    chain_spec::{ChainSpec, SyncChainSpec},
    state::StateOverrides,
    transaction,
};
use edr_rpc_eth::{CallRequest, EstimateGasRequest};

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError};

pub trait ProviderSpec<TimerT: Clone + TimeSinceEpoch>:
    ChainSpec<
    Hardfork: Debug,
    RpcCallRequest: MaybeSender + ResolveRpcType<Self, TimerT, Self::TransactionRequest>,
    RpcEstimateGasRequest: MaybeSender + ResolveRpcType<Self, TimerT, Self::TransactionRequest>,
>
{
    /// Type representing a transaction request.
    type TransactionRequest: FakeSign<Signed = Self::Transaction> + Sign<Signed = Self::Transaction>;
}

impl<TimerT: Clone + TimeSinceEpoch> ProviderSpec<TimerT> for L1ChainSpec {
    type TransactionRequest = transaction::Request;
}

pub trait MaybeSender {
    fn maybe_sender(&self) -> Option<&Address>;
}

impl MaybeSender for CallRequest {
    fn maybe_sender(&self) -> Option<&Address> {
        self.from.as_ref()
    }
}

impl MaybeSender for EstimateGasRequest {
    fn maybe_sender(&self) -> Option<&Address> {
        self.inner.from.as_ref()
    }
}

/// Trait for resolving an RPC type to an internal type.
pub trait ResolveRpcType<
    ChainSpecT: ProviderSpec<TimerT, Hardfork: Debug>,
    TimerT: Clone + TimeSinceEpoch,
    OutputT,
>
{
    fn resolve_rpc_type(
        self,
        data: &mut ProviderData<ChainSpecT, TimerT>,
        block_spec: &BlockSpec,
        state_overrides: &StateOverrides,
    ) -> Result<OutputT, ProviderError<ChainSpecT>>;
}

pub trait SyncProviderSpec<TimerT: Clone + TimeSinceEpoch>:
    ProviderSpec<TimerT> + SyncChainSpec
{
}

impl<ProviderSpecT: ProviderSpec<TimerT> + SyncChainSpec, TimerT: Clone + TimeSinceEpoch>
    SyncProviderSpec<TimerT> for ProviderSpecT
{
}
