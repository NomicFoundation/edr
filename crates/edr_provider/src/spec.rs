use std::sync::Arc;

use edr_block_api::{BlockAndTotalDifficulty, GenesisBlockFactory, SyncGenesisBlockFactory};
use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec,
};
use edr_chain_spec::ExecutableTransaction;
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_provider::ProviderChainSpec;
use edr_eth::{Blob, BlockSpec};
use edr_evm::overrides::StateOverrides;
use edr_primitives::{Address, B256};
use edr_signer::{FakeSign, Sign};
use edr_transaction::IsSupported;

use crate::{
    data::ProviderData, error::ProviderErrorForChainSpec, time::TimeSinceEpoch,
    TransactionFailureReason,
};

pub trait ProviderSpec<TimerT: Clone + TimeSinceEpoch>:
    GenesisBlockFactory<LocalBlock = <Self as BlockChainSpec>::LocalBlock>
    + ProviderChainSpec<
        // Block: FetchBlockReceipts<Arc<Self::Receipt>, Error = BlockchainErrorForChainSpec<Self>>,
        // LocalBlock: FetchBlockReceipts<
        //     Arc<Self::Receipt>,
        //     Error = BlockchainErrorForChainSpec<Self>,
        // >,
        RpcBlock<B256>: From<BlockAndTotalDifficulty<Arc<Self::Block>, Self::SignedTransaction>>,
        RpcCallRequest: MaybeSender,
        RpcTransactionRequest: Sender,
        SignedTransaction: IsSupported,
    >
{
    type PooledTransaction: HardforkValidationData
        + Into<Self::SignedTransaction>
        + alloy_rlp::Decodable
        + ExecutableTransaction;

    /// Type representing a transaction request.
    type TransactionRequest: FakeSign<Signed = Self::SignedTransaction>
        + Sign<Signed = Self::SignedTransaction>
        + for<'context> FromRpcType<
            Self::RpcCallRequest,
            TimerT,
            Context<'context> = CallContext<'context, Self, TimerT>,
            Error = ProviderErrorForChainSpec<Self>,
        > + for<'context> FromRpcType<
            Self::RpcTransactionRequest,
            TimerT,
            Context<'context> = TransactionContext<'context, Self, TimerT>,
            Error = ProviderErrorForChainSpec<Self>,
        >;

    /// Casts a halt reason into a transaction failure reason.
    ///
    /// This is implemented as an associated function to avoid problems when
    /// implementing type conversions for third-party types.
    fn cast_halt_reason(reason: Self::HaltReason) -> TransactionFailureReason<Self::HaltReason>;
}

impl<TimerT: Clone + TimeSinceEpoch> ProviderSpec<TimerT> for L1ChainSpec {
    type PooledTransaction = edr_chain_l1::L1PooledTransaction;
    type TransactionRequest = edr_chain_l1::L1TransactionRequest;

    fn cast_halt_reason(reason: Self::HaltReason) -> TransactionFailureReason<Self::HaltReason> {
        match reason {
            Self::HaltReason::CreateContractSizeLimit => {
                TransactionFailureReason::CreateContractSizeLimit
            }
            Self::HaltReason::OpcodeNotFound | Self::HaltReason::InvalidFEOpcode => {
                TransactionFailureReason::OpcodeNotFound
            }
            Self::HaltReason::OutOfGas(error) => TransactionFailureReason::OutOfGas(error),
            remainder => TransactionFailureReason::Inner(remainder),
        }
    }
}

/// Trait with data used for validating a transaction complies with a
/// [`edr_chain_spec::EvmSpecId`].
pub trait HardforkValidationData {
    /// Returns the `to` address of the transaction.
    fn to(&self) -> Option<&Address>;

    /// Returns the gas price of the transaction.
    fn gas_price(&self) -> Option<&u128>;

    /// Returns the max fee per gas of the transaction.
    fn max_fee_per_gas(&self) -> Option<&u128>;

    /// Returns the max priority fee per gas of the transaction.
    fn max_priority_fee_per_gas(&self) -> Option<&u128>;

    /// Returns the access list of the transaction.
    fn access_list(&self) -> Option<&Vec<edr_eip2930::AccessListItem>>;

    /// Returns the blobs of the transaction.
    fn blobs(&self) -> Option<&Vec<Blob>>;

    /// Returns the blob hashes of the transaction.
    fn blob_hashes(&self) -> Option<&Vec<B256>>;

    /// Returns the authorization list of the transaction.
    fn authorization_list(&self) -> Option<&Vec<edr_eip7702::SignedAuthorization>>;
}

/// Trait for retrieving the sender of a request, if any.
pub trait MaybeSender {
    /// Retrieves the sender of the request, if any.
    fn maybe_sender(&self) -> Option<&Address>;
}

impl MaybeSender for L1CallRequest {
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
pub trait FromRpcType<RpcT, TimerT: Clone + TimeSinceEpoch>: Sized {
    /// Type for contextual information.
    type Context<'context>;

    /// Type of error that can occur during resolution.
    type Error;

    fn from_rpc_type(value: RpcT, context: Self::Context<'_>) -> Result<Self, Self::Error>;
}

pub trait SyncProviderSpec<TimerT: Clone + TimeSinceEpoch>:
    ProviderSpec<TimerT> + SyncGenesisBlockFactory<LocalBlock = <Self as BlockChainSpec>::LocalBlock>
{
}

impl<
        ProviderSpecT: ProviderSpec<TimerT>
            + SyncGenesisBlockFactory<LocalBlock = <Self as BlockChainSpec>::LocalBlock>,
        TimerT: Clone + TimeSinceEpoch,
    > SyncProviderSpec<TimerT> for ProviderSpecT
{
}

pub type DefaultGasPriceFn<ChainSpecT, TimerT> =
    fn(&ProviderData<ChainSpecT, TimerT>) -> Result<u128, ProviderErrorForChainSpec<ChainSpecT>>;

pub type MaxFeesFn<ChainSpecT, TimerT> =
    fn(
        &ProviderData<ChainSpecT, TimerT>,
        // block_spec
        &BlockSpec,
        // max_fee_per_gas
        Option<u128>,
        // max_priority_fee_per_gas
        Option<u128>,
    ) -> Result<(u128, u128), ProviderErrorForChainSpec<ChainSpecT>>;

pub struct CallContext<'context, ChainSpecT: ProviderSpec<TimerT>, TimerT: Clone + TimeSinceEpoch> {
    pub data: &'context mut ProviderData<ChainSpecT, TimerT>,
    pub block_spec: &'context BlockSpec,
    pub state_overrides: &'context StateOverrides,
    pub default_gas_price_fn: DefaultGasPriceFn<ChainSpecT, TimerT>,
    pub max_fees_fn: MaxFeesFn<ChainSpecT, TimerT>,
}

pub struct TransactionContext<
    'context,
    ChainSpecT: ProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
> {
    pub data: &'context mut ProviderData<ChainSpecT, TimerT>,
}
