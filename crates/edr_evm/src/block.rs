mod builder;
mod local;
mod remote;
/// Block-related transaction types.
pub mod transaction;

use std::sync::Arc;

use edr_block_api::{Block, BlockAndTotalDifficulty, BlockReceipts};
use edr_block_header::{BlobGas, BlockHeader, Withdrawal};
use edr_block_storage::ReservableSparseBlockStorage;
use edr_chain_l1::rpc::block::L1RpcBlock;
use edr_chain_spec::{ChainHardfork, ChainSpec};
use edr_primitives::B256;
use edr_receipt::ReceiptTrait;

pub use self::{
    builder::{
        BlockBuilder, BlockBuilderCreationError, BlockBuilderCreationErrorForChainSpec,
        BlockInputs, BlockTransactionError, BlockTransactionErrorForChainSpec, EthBlockBuilder,
        EthBlockReceiptFactory, GenesisBlockOptions,
    },
    local::{CreationError as LocalCreationError, EthLocalBlock, EthLocalBlockForChainSpec},
    remote::{ConversionError as RemoteBlockConversionError, RemoteBlock},
};

/// Helper type for a chain-specific [`ReservableSparseBlockStorage`].
pub type ReservableSparseBlockStorageForChainSpec<ChainSpecT> = ReservableSparseBlockStorage<
    Arc<<ChainSpecT as RuntimeSpec>::BlockReceipt>,
    Arc<<ChainSpecT as RuntimeSpec>::LocalBlock>,
    <ChainSpecT as ChainHardfork>::Hardfork,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

/// Trait that meets all requirements for an Ethereum block.
pub trait EthBlock<BlockReceiptT: ReceiptTrait, SignedTransactionT>:
    Block<SignedTransactionT> + BlockReceipts<BlockReceiptT>
{
}

impl<BlockReceiptT, BlockT, SignedTransactionT> EthBlock<BlockReceiptT, SignedTransactionT>
    for BlockT
where
    BlockReceiptT: ReceiptTrait,
    BlockT: Block<SignedTransactionT> + BlockReceipts<BlockReceiptT>,
{
}

/// Trait that meets all requirements for a synchronous block.
pub trait SyncBlock<BlockReceiptT: ReceiptTrait, SignedTransactionT>:
    EthBlock<BlockReceiptT, SignedTransactionT> + Send + Sync
{
}

impl<BlockReceiptT, BlockT, SignedTransactionT> SyncBlock<BlockReceiptT, SignedTransactionT>
    for BlockT
where
    BlockReceiptT: ReceiptTrait,
    BlockT: EthBlock<BlockReceiptT, SignedTransactionT> + Send + Sync,
{
}

impl<ChainSpecT: RuntimeSpec> TryFrom<L1RpcBlock<ChainSpecT::RpcTransaction>>
    for EthBlockData<ChainSpecT>
{
    type Error = RemoteBlockConversionError<ChainSpecT::RpcTransactionConversionError>;

    fn try_from(value: L1RpcBlock<ChainSpecT::RpcTransaction>) -> Result<Self, Self::Error> {
        let header = BlockHeader {
            parent_hash: value.parent_hash,
            ommers_hash: value.sha3_uncles,
            beneficiary: value
                .miner
                .ok_or(RemoteBlockConversionError::MissingMiner)?,
            state_root: value.state_root,
            transactions_root: value.transactions_root,
            receipts_root: value.receipts_root,
            logs_bloom: value.logs_bloom,
            difficulty: value.difficulty,
            number: value
                .number
                .ok_or(RemoteBlockConversionError::MissingNumber)?,
            gas_limit: value.gas_limit,
            gas_used: value.gas_used,
            timestamp: value.timestamp,
            extra_data: value.extra_data,
            // TODO don't accept remote blocks with missing mix hash,
            // see https://github.com/NomicFoundation/edr/issues/518
            mix_hash: value.mix_hash.unwrap_or_default(),
            nonce: value
                .nonce
                .ok_or(RemoteBlockConversionError::MissingNonce)?,
            base_fee_per_gas: value.base_fee_per_gas,
            withdrawals_root: value.withdrawals_root,
            blob_gas: value.blob_gas_used.and_then(|gas_used| {
                value.excess_blob_gas.map(|excess_gas| BlobGas {
                    gas_used,
                    excess_gas,
                })
            }),
            parent_beacon_block_root: value.parent_beacon_block_root,
            requests_hash: value.requests_hash,
        };

        let transactions = value
            .transactions
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<ChainSpecT::SignedTransaction>, _>>()
            .map_err(RemoteBlockConversionError::TransactionConversionError)?;

        let hash = value.hash.ok_or(RemoteBlockConversionError::MissingHash)?;

        Ok(Self {
            header,
            transactions,
            ommer_hashes: value.uncles,
            withdrawals: value.withdrawals,
            hash,
            rlp_size: value.size,
        })
    }
}

/// Helper type for a chain-specific [`BlockAndTotalDifficulty`].
pub type BlockAndTotalDifficultyForChainSpec<ChainSpecT> = BlockAndTotalDifficulty<
    Arc<<ChainSpecT as RuntimeSpec>::Block>,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;
