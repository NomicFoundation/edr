use std::fmt::Debug;

use edr_eth::{
    block::{self, BlobGas, PartialHeader},
    chain_spec::{EthHeaderConstants, L1ChainSpec},
    env::{BlobExcessGasAndPrice, BlockEnv},
    log::{ExecutionLog, FilterLog},
    receipt::{ExecutionReceiptBuilder, MapReceiptLogs},
    result::InvalidTransaction,
    transaction::ExecutableTransaction,
    SpecId, B256, U256,
};
use edr_rpc_eth::{spec::RpcSpec, RpcTypeFrom, TransactionConversionError};
use revm::primitives::TransactionValidation;
pub use revm::EvmWiring;

use crate::{
    block::transaction::TransactionAndBlock,
    hardfork::{self, Activations},
    transaction::{remote::EthRpcTransaction, Transaction, TransactionError, TransactionType},
    BlockReceipt, EthBlockData, EthRpcBlock, RemoteBlockConversionError,
};

/// A trait for defining a chain's associated types.
// Bug: https://github.com/rust-lang/rust-clippy/issues/12927
#[allow(clippy::trait_duplication_in_bounds)]
pub trait ChainSpec:
    alloy_rlp::Encodable
    + EthHeaderConstants
    + EvmWiring<
        Block: BlockEnvConstructor<Self, block::Header> + BlockEnvConstructor<Self, PartialHeader>,
        Transaction: alloy_rlp::Encodable
                         + Clone
                         + Debug
                         + PartialEq
                         + Eq
                         + ExecutableTransaction
                         + Transaction
                         + TransactionType
                         + TryFrom<
            <Self as RpcSpec>::RpcTransaction,
            Error = Self::RpcTransactionConversionError,
        >,
    > + RpcSpec<
        ExecutionReceipt<FilterLog>: Debug,
        RpcBlock<<Self as RpcSpec>::RpcTransaction>: EthRpcBlock
                                                         + TryInto<
            EthBlockData<Self>,
            Error = Self::RpcBlockConversionError,
        >,
        RpcReceipt: Debug
                        + RpcTypeFrom<BlockReceipt<Self>, Hardfork = Self::Hardfork>
                        + TryInto<BlockReceipt<Self>, Error = Self::RpcReceiptConversionError>,
        RpcTransaction: EthRpcTransaction
                            + RpcTypeFrom<TransactionAndBlock<Self>, Hardfork = Self::Hardfork>,
    > + RpcSpec<
        ExecutionReceipt<ExecutionLog>: alloy_rlp::Encodable
                                            + MapReceiptLogs<
            ExecutionLog,
            FilterLog,
            Self::ExecutionReceipt<FilterLog>,
        >,
        RpcBlock<B256>: EthRpcBlock,
    >
{
    /// Type representing a builder that constructs an execution receipt.
    type ReceiptBuilder: ExecutionReceiptBuilder<
        Self,
        Receipt = Self::ExecutionReceipt<ExecutionLog>,
    >;

    /// Type representing an error that occurs when converting an RPC block.
    type RpcBlockConversionError: std::error::Error;

    /// Type representing an error that occurs when converting an RPC receipt.
    type RpcReceiptConversionError: std::error::Error;

    /// Type representing an error that occurs when converting an RPC
    /// transaction.
    type RpcTransactionConversionError: std::error::Error;

    /// Casts a transaction validation error into a `TransactionError`.
    ///
    /// This is implemented as an associated function to avoid problems when
    /// implementing type conversions for third-party types.
    fn cast_transaction_error<BlockchainErrorT, StateErrorT>(
        error: <Self::Transaction as TransactionValidation>::ValidationError,
    ) -> TransactionError<Self, BlockchainErrorT, StateErrorT>;

    /// Returns the hardfork activations corresponding to the provided chain ID,
    /// if it is associated with this chain specification.
    fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<Self>>;

    /// Returns the name corresponding to the provided chain ID, if it is
    /// associated with this chain specification.
    fn chain_name(chain_id: u64) -> Option<&'static str>;
}

/// A trait for constructing a block a [`PartialHeader`] into an EVM block.
pub trait BlockEnvConstructor<ChainSpecT: ChainSpec, HeaderT> {
    /// Converts the instance into an EVM block.
    fn new_block_env(header: &HeaderT, hardfork: ChainSpecT::Hardfork) -> Self;
}

impl BlockEnvConstructor<L1ChainSpec, PartialHeader> for BlockEnv {
    fn new_block_env(header: &PartialHeader, hardfork: SpecId) -> Self {
        Self {
            number: U256::from(header.number),
            coinbase: header.beneficiary,
            timestamp: U256::from(header.timestamp),
            difficulty: header.difficulty,
            basefee: header.base_fee.unwrap_or(U256::ZERO),
            gas_limit: U256::from(header.gas_limit),
            prevrandao: if hardfork >= SpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header
                .blob_gas
                .as_ref()
                .map(|BlobGas { excess_gas, .. }| BlobExcessGasAndPrice::new(*excess_gas)),
        }
    }
}

impl BlockEnvConstructor<L1ChainSpec, block::Header> for BlockEnv {
    fn new_block_env(header: &block::Header, hardfork: SpecId) -> Self {
        Self {
            number: U256::from(header.number),
            coinbase: header.beneficiary,
            timestamp: U256::from(header.timestamp),
            difficulty: header.difficulty,
            basefee: header.base_fee_per_gas.unwrap_or(U256::ZERO),
            gas_limit: U256::from(header.gas_limit),
            prevrandao: if hardfork >= SpecId::MERGE {
                Some(header.mix_hash)
            } else {
                None
            },
            blob_excess_gas_and_price: header
                .blob_gas
                .as_ref()
                .map(|BlobGas { excess_gas, .. }| BlobExcessGasAndPrice::new(*excess_gas)),
        }
    }
}

/// A supertrait for [`ChainSpec`] that is safe to send between threads.
pub trait SyncChainSpec:
    ChainSpec<
        ExecutionReceipt<FilterLog>: Send + Sync,
        HaltReason: Send + Sync,
        Hardfork: Send + Sync,
        RpcBlockConversionError: Send + Sync,
        RpcReceiptConversionError: Send + Sync,
        Transaction: TransactionValidation<ValidationError: Send + Sync> + Send + Sync,
    > + Send
    + Sync
    + 'static
{
}

impl<ChainSpecT> SyncChainSpec for ChainSpecT where
    ChainSpecT: ChainSpec<
            ExecutionReceipt<FilterLog>: Send + Sync,
            HaltReason: Send + Sync,
            Hardfork: Send + Sync,
            RpcBlockConversionError: Send + Sync,
            RpcReceiptConversionError: Send + Sync,
            Transaction: TransactionValidation<ValidationError: Send + Sync> + Send + Sync,
        > + Send
        + Sync
        + 'static
{
}

impl ChainSpec for L1ChainSpec {
    type ReceiptBuilder = edr_eth::receipt::execution::Builder;
    type RpcBlockConversionError = RemoteBlockConversionError<Self>;
    type RpcReceiptConversionError = edr_rpc_eth::receipt::ConversionError;
    type RpcTransactionConversionError = TransactionConversionError;

    fn cast_transaction_error<BlockchainErrorT, StateErrorT>(
        error: <Self::Transaction as TransactionValidation>::ValidationError,
    ) -> TransactionError<Self, BlockchainErrorT, StateErrorT> {
        match error {
            InvalidTransaction::LackOfFundForMaxFee { fee, balance } => {
                TransactionError::LackOfFundForMaxFee { fee, balance }
            }
            remainder => TransactionError::InvalidTransaction(remainder),
        }
    }

    fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<Self>> {
        hardfork::l1::chain_hardfork_activations(chain_id)
    }

    fn chain_name(chain_id: u64) -> Option<&'static str> {
        hardfork::l1::chain_name(chain_id)
    }
}
