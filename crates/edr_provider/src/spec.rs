use core::fmt::Debug;

use edr_eth::{
    chain_spec::L1ChainSpec,
    rlp,
    transaction::{
        signed::{FakeSign, Sign},
        Transaction,
    },
    AccessListItem, Address, Blob, BlockSpec, B256, U256,
};
use edr_evm::{
    chain_spec::{ChainSpec, SyncChainSpec},
    state::StateOverrides,
    transaction,
};
use edr_rpc_eth::{CallRequest, TransactionRequest};

use crate::{data::ProviderData, time::TimeSinceEpoch, ProviderError, TransactionFailureReason};

pub trait ProviderSpec<TimerT: Clone + TimeSinceEpoch>:
    ChainSpec<
    HaltReason: Into<TransactionFailureReason<Self>>,
    Hardfork: Debug,
    RpcCallRequest: MaybeSender
                        + for<'context> ResolveRpcType<
        TimerT,
        Self::TransactionRequest,
        Context<'context> = CallContext<'context, Self, TimerT>,
        Error = ProviderError<Self>,
    >,
    RpcTransactionRequest: Sender
                               + for<'context> ResolveRpcType<
        TimerT,
        Self::TransactionRequest,
        Context<'context> = TransactionContext<'context, Self, TimerT>,
        Error = ProviderError<Self>,
    >,
>
{
    type PooledTransaction: HardforkValidationData
        + Into<Self::Transaction>
        + rlp::Decodable
        + Transaction;

    /// Type representing a transaction request.
    type TransactionRequest: FakeSign<Signed = Self::Transaction> + Sign<Signed = Self::Transaction>;
}

impl<TimerT: Clone + TimeSinceEpoch> ProviderSpec<TimerT> for L1ChainSpec {
    type PooledTransaction = transaction::pooled::PooledTransaction;
    type TransactionRequest = transaction::Request;
}

/// Trait with data used for validating a transaction complies with a
/// [`SpecId`].
pub trait HardforkValidationData {
    /// Returns the `to` address of the transaction.
    fn to(&self) -> Option<&Address>;

    /// Returns the gas price of the transaction.
    fn gas_price(&self) -> Option<&U256>;

    /// Returns the max fee per gas of the transaction.
    fn max_fee_per_gas(&self) -> Option<&U256>;

    /// Returns the max priority fee per gas of the transaction.
    fn max_priority_fee_per_gas(&self) -> Option<&U256>;

    /// Returns the access list of the transaction.
    fn access_list(&self) -> Option<&Vec<AccessListItem>>;

    /// Returns the blobs of the transaction.
    fn blobs(&self) -> Option<&Vec<Blob>>;

    /// Returns the blob hashes of the transaction.
    fn blob_hashes(&self) -> Option<&Vec<B256>>;
}

/// Trait for retrieving the sender of a request, if any.
pub trait MaybeSender {
    /// Retrieves the sender of the request, if any.
    fn maybe_sender(&self) -> Option<&Address>;
}

impl MaybeSender for CallRequest {
    fn maybe_sender(&self) -> Option<&Address> {
        self.from.as_ref()
    }
}

/// Trait for retrieving the sender of a request.
pub trait Sender {
    /// Retrieves the sender of the request.
    fn sender(&self) -> &Address;
}

impl Sender for TransactionRequest {
    fn sender(&self) -> &Address {
        &self.from
    }
}

// ChainSpecT: ProviderSpec<TimerT, Hardfork: Debug>,

/// Trait for resolving an RPC type to an internal type.
pub trait ResolveRpcType<TimerT: Clone + TimeSinceEpoch, OutputT> {
    /// Type for contextual information.
    type Context<'context>;

    /// Type of error that can occur during resolution.
    type Error;

    fn resolve_rpc_type(self, context: Self::Context<'_>) -> Result<OutputT, Self::Error>;
}

pub trait SyncProviderSpec<TimerT: Clone + TimeSinceEpoch>:
    ProviderSpec<TimerT> + SyncChainSpec
{
}

impl<ProviderSpecT: ProviderSpec<TimerT> + SyncChainSpec, TimerT: Clone + TimeSinceEpoch>
    SyncProviderSpec<TimerT> for ProviderSpecT
{
}

pub type DefaultGasPriceFn<ChainSpecT, TimerT> =
    fn(&ProviderData<ChainSpecT, TimerT>) -> Result<U256, ProviderError<ChainSpecT>>;

pub type MaxFeesFn<ChainSpecT, TimerT> = fn(
    &ProviderData<ChainSpecT, TimerT>,
    // block_spec
    &BlockSpec,
    // max_fee_per_gas
    Option<U256>,
    // max_priority_fee_per_gas
    Option<U256>,
) -> Result<(U256, U256), ProviderError<ChainSpecT>>;

pub struct CallContext<
    'context,
    ChainSpecT: ProviderSpec<TimerT, Hardfork: Debug>,
    TimerT: Clone + TimeSinceEpoch,
> {
    pub data: &'context mut ProviderData<ChainSpecT, TimerT>,
    pub block_spec: &'context BlockSpec,
    pub state_overrides: &'context StateOverrides,
    pub default_gas_price_fn: DefaultGasPriceFn<ChainSpecT, TimerT>,
    pub max_fees_fn: MaxFeesFn<ChainSpecT, TimerT>,
}

pub struct TransactionContext<
    'context,
    ChainSpecT: ProviderSpec<TimerT, Hardfork: Debug>,
    TimerT: Clone + TimeSinceEpoch,
> {
    pub data: &'context mut ProviderData<ChainSpecT, TimerT>,
}
