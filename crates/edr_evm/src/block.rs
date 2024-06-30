mod builder;
mod local;
mod remote;

use std::{fmt::Debug, sync::Arc};

use auto_impl::auto_impl;
use edr_eth::{
    block::{self, BlobGas, Header},
    receipt::BlockReceipt,
    transaction::SignedTransaction,
    withdrawal::Withdrawal,
    B256, U256,
};

pub use self::{
    builder::{
        BlockBuilder, BlockBuilderCreationError, BlockTransactionError, BuildBlockResult,
        ExecutionResultWithContext,
    },
    local::LocalBlock,
    remote::{ConversionError as RemoteBlockConversionError, EthRpcBlock, RemoteBlock},
};
use crate::chain_spec::ChainSpec;

/// Trait for implementations of an Ethereum block.
#[auto_impl(Arc)]
pub trait Block<ChainSpecT: ChainSpec>: Debug {
    /// The blockchain error type.
    type Error;

    /// Returns the block's hash.
    fn hash(&self) -> &B256;

    /// Returns the block's header.
    fn header(&self) -> &block::Header;

    /// Ommer/uncle block hashes.
    fn ommer_hashes(&self) -> &[B256];

    /// The length of the RLP encoding of this block in bytes.
    fn rlp_size(&self) -> u64;

    /// Returns the block's transactions.
    fn transactions(&self) -> &[ChainSpecT::Transaction];

    /// Returns the receipts of the block's transactions.
    fn transaction_receipts(&self) -> Result<Vec<Arc<BlockReceipt>>, Self::Error>;

    /// Withdrawals
    fn withdrawals(&self) -> Option<&[Withdrawal]>;
}

/// Trait that meets all requirements for a synchronous block.
pub trait SyncBlock<ChainSpecT>: Block<ChainSpecT> + Send + Sync
where
    ChainSpecT: ChainSpec,
{
}

impl<BlockT, ChainSpecT> SyncBlock<ChainSpecT> for BlockT
where
    BlockT: Block<ChainSpecT> + Send + Sync,
    ChainSpecT: ChainSpec,
{
}

/// A type containing the relevant data for an Ethereum block.
pub struct EthBlockData<ChainSpecT: ChainSpec> {
    /// The block's header.
    pub header: edr_eth::block::Header,
    /// The block's transactions.
    pub transactions: Vec<ChainSpecT::Transaction>,
    /// The hashes of the block's ommers.
    pub ommer_hashes: Vec<B256>,
    /// The staking withdrawals.
    pub withdrawals: Option<Vec<Withdrawal>>,
    /// The block's hash.
    pub hash: B256,
    /// The length of the RLP encoding of this block in bytes.
    pub rlp_size: u64,
}

impl<ChainSpecT: ChainSpec> TryFrom<edr_rpc_eth::Block<ChainSpecT::RpcTransaction>>
    for EthBlockData<ChainSpecT>
{
    type Error = RemoteBlockConversionError<ChainSpecT>;

    fn try_from(
        value: edr_rpc_eth::Block<ChainSpecT::RpcTransaction>,
    ) -> Result<Self, Self::Error> {
        let header = Header {
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
        };

        let transactions = value
            .transactions
            .into_iter()
            .map(ChainSpecT::Transaction::try_from)
            .collect::<Result<Vec<_>, _>>()
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

/// The result returned by requesting a block by number.
#[derive(Debug)]
pub struct BlockAndTotalDifficulty<ChainSpecT: ChainSpec, BlockchainErrorT> {
    /// The block
    pub block: Arc<dyn SyncBlock<ChainSpecT, Error = BlockchainErrorT>>,
    /// The total difficulty with the block
    pub total_difficulty: Option<U256>,
}

impl<BlockchainErrorT, ChainSpecT: ChainSpec> Clone
    for BlockAndTotalDifficulty<ChainSpecT, BlockchainErrorT>
{
    fn clone(&self) -> Self {
        Self {
            block: self.block.clone(),
            total_difficulty: self.total_difficulty,
        }
    }
}

impl<BlockchainErrorT, ChainSpecT: ChainSpec>
    From<BlockAndTotalDifficulty<ChainSpecT, BlockchainErrorT>> for edr_rpc_eth::Block<B256>
{
    fn from(value: BlockAndTotalDifficulty<ChainSpecT, BlockchainErrorT>) -> Self {
        let transactions = value
            .block
            .transactions()
            .iter()
            .map(|tx| *tx.transaction_hash())
            .collect();

        let header = value.block.header();
        edr_rpc_eth::Block {
            hash: Some(*value.block.hash()),
            parent_hash: header.parent_hash,
            sha3_uncles: header.ommers_hash,
            state_root: header.state_root,
            transactions_root: header.transactions_root,
            receipts_root: header.receipts_root,
            number: Some(header.number),
            gas_used: header.gas_used,
            gas_limit: header.gas_limit,
            extra_data: header.extra_data.clone(),
            logs_bloom: header.logs_bloom,
            timestamp: header.timestamp,
            difficulty: header.difficulty,
            total_difficulty: value.total_difficulty,
            uncles: value.block.ommer_hashes().to_vec(),
            transactions,
            size: value.block.rlp_size(),
            mix_hash: Some(header.mix_hash),
            nonce: Some(header.nonce),
            base_fee_per_gas: header.base_fee_per_gas,
            miner: Some(header.beneficiary),
            withdrawals: value
                .block
                .withdrawals()
                .map(<[edr_eth::withdrawal::Withdrawal]>::to_vec),
            withdrawals_root: header.withdrawals_root,
            blob_gas_used: header.blob_gas.as_ref().map(|bg| bg.gas_used),
            excess_blob_gas: header.blob_gas.as_ref().map(|bg| bg.excess_gas),
            parent_beacon_block_root: header.parent_beacon_block_root,
        }
    }
}
