//! Test utilities for blockchain-related tests.
#![warn(missing_docs)]

#[macro_use]
mod macros;

use core::fmt::Debug;
use std::{collections::BTreeMap, sync::Arc};

use edr_block_api::{sync::SyncBlock, BlockAndTotalDifficulty, EmptyBlock as _};
use edr_block_header::{BlockConfig, HeaderOverrides, PartialHeader};
use edr_block_local::EthLocalBlock;
use edr_chain_l1::{receipt::builder::L1ExecutionReceiptBuilder, L1ChainSpec};
use edr_chain_spec::{ChainSpec, ExecutableTransaction as _, HardforkChainSpec};
use edr_chain_spec_block::BlockChainSpec;
use edr_chain_spec_provider::ProviderChainSpec as _;
use edr_evm_spec::result::{ExecutionResult, Output, SuccessReason};
use edr_primitives::{Address, Bytes, B256, U256};
use edr_provider::spec::BlockchainForChainSpec;
use edr_receipt::{log::ExecutionLog, TransactionReceipt};
use edr_receipt_builder_api::ExecutionReceiptBuilder as _;
use edr_receipt_spec::ReceiptChainSpec;
use edr_state_api::StateDiff;
use edr_test_transaction::dummy_eip155_transaction;
// Re-export types that are used by the macros.
pub use paste;

/// Helper type for a chain-specific [`BlockAndTotalDifficulty`].
pub type BlockAndTotalDifficultyForChainSpec<ChainSpecT> = BlockAndTotalDifficulty<
    Arc<<ChainSpecT as BlockChainSpec>::Block>,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

/// Helper type for a chain-specific [`SyncBlock`].
pub type DynSyncBlock<ChainSpecT> = dyn SyncBlock<
    Arc<<ChainSpecT as ReceiptChainSpec>::Receipt>,
    <ChainSpecT as ChainSpec>::SignedTransaction,
    Error = <ChainSpecT as BlockChainSpec>::FetchReceiptError,
>;

/// Helper type for a chain-specific [`EthLocalBlock`].
pub type EthLocalBlockForChainSpec<ChainSpecT> = EthLocalBlock<
    <ChainSpecT as ReceiptChainSpec>::Receipt,
    <ChainSpecT as BlockChainSpec>::FetchReceiptError,
    <ChainSpecT as HardforkChainSpec>::Hardfork,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;

/// Creates a dummy block for the provided blockchain.
pub fn create_dummy_block<BlockchainErrorT: Debug>(
    blockchain: &dyn BlockchainForChainSpec<L1ChainSpec, BlockchainErrorT>,
) -> EthLocalBlockForChainSpec<L1ChainSpec> {
    let block_number = blockchain.last_block_number() + 1;

    create_dummy_block_with_number(blockchain, block_number)
}

/// Creates a dummy block with the specified block number for the provided
/// blockchain.
pub fn create_dummy_block_with_number<BlockchainErrorT: Debug>(
    blockchain: &dyn BlockchainForChainSpec<L1ChainSpec, BlockchainErrorT>,
    number: u64,
) -> EthLocalBlockForChainSpec<L1ChainSpec> {
    let parent_hash = *blockchain
        .last_block()
        .expect("Failed to retrieve last block")
        .block_hash();

    create_dummy_block_with_hash(blockchain, number, parent_hash)
}

/// Creates a dummy block with the specified block number and difficulty for the
/// provided blockchain.
pub fn create_dummy_block_with_difficulty<BlockchainErrorT: Debug>(
    blockchain: &dyn BlockchainForChainSpec<L1ChainSpec, BlockchainErrorT>,
    number: u64,
    difficulty: u64,
) -> EthLocalBlockForChainSpec<L1ChainSpec> {
    let parent_hash = *blockchain
        .last_block()
        .expect("Failed to retrieve last block")
        .block_hash();

    create_dummy_block_with_header(
        blockchain.hardfork(),
        PartialHeader::new::<edr_chain_l1::Hardfork>(
            BlockConfig {
                base_fee_params: blockchain.base_fee_params(),
                hardfork: blockchain.hardfork(),
                min_ethash_difficulty: L1ChainSpec::MIN_ETHASH_DIFFICULTY,
            },
            HeaderOverrides {
                parent_hash: Some(parent_hash),
                number: Some(number),
                difficulty: Some(U256::from(difficulty)),
                ..HeaderOverrides::default()
            },
            None,
            &Vec::new(),
            None,
        ),
    )
}

/// Creates a dummy block with the specified block number and parent hash for
/// the provided blockchain.
pub fn create_dummy_block_with_hash<BlockchainErrorT>(
    blockchain: &dyn BlockchainForChainSpec<L1ChainSpec, BlockchainErrorT>,
    number: u64,
    parent_hash: B256,
) -> EthLocalBlockForChainSpec<L1ChainSpec> {
    create_dummy_block_with_header(
        blockchain.hardfork(),
        PartialHeader::new::<edr_chain_l1::Hardfork>(
            BlockConfig {
                base_fee_params: blockchain.base_fee_params(),
                hardfork: blockchain.hardfork(),
                min_ethash_difficulty: L1ChainSpec::MIN_ETHASH_DIFFICULTY,
            },
            HeaderOverrides {
                parent_hash: Some(parent_hash),
                number: Some(number),
                ..HeaderOverrides::default()
            },
            None,
            &Vec::new(),
            None,
        ),
    )
}

/// Creates an empty dummy block with the specified header for the provided
/// hardfork.
pub fn create_dummy_block_with_header(
    hardfork: edr_chain_l1::Hardfork,
    partial_header: PartialHeader,
) -> EthLocalBlockForChainSpec<L1ChainSpec> {
    EthLocalBlock::empty(hardfork, partial_header)
}

/// A dummy block along with the contained singular transaction and its receipt.
pub struct DummyBlockAndTransaction {
    /// The mined dummy block.
    pub block: Arc<<L1ChainSpec as BlockChainSpec>::Block>,
    /// The hash of the singular mined dummy transaction.
    pub transaction_hash: B256,
    /// The receipt of the singular mined dummy transaction.
    pub transaction_receipt:
        TransactionReceipt<edr_chain_l1::TypedEnvelope<edr_receipt::Execution<ExecutionLog>>>,
}

/// Returns the transaction's hash.
pub fn insert_dummy_block_with_transaction<
    BlockchainErrorT: 'static + Send + Sync + std::error::Error,
>(
    blockchain: &mut dyn BlockchainForChainSpec<L1ChainSpec, BlockchainErrorT>,
) -> anyhow::Result<DummyBlockAndTransaction> {
    const GAS_USED: u64 = 100;

    let caller = Address::random();
    let transaction = dummy_eip155_transaction(caller, 0)?;
    let transaction_hash = *transaction.transaction_hash();

    let mut header = PartialHeader::new::<edr_chain_l1::Hardfork>(
        BlockConfig {
            base_fee_params: blockchain.base_fee_params(),
            hardfork: blockchain.hardfork(),
            min_ethash_difficulty: L1ChainSpec::MIN_ETHASH_DIFFICULTY,
        },
        HeaderOverrides::default(),
        Some(blockchain.last_block()?.block_header()),
        &Vec::new(),
        None,
    );
    header.gas_used = GAS_USED;

    let state_overrides = BTreeMap::new();
    let state = blockchain.state_at_block_number(header.number - 1, &state_overrides)?;

    let receipt_builder = L1ExecutionReceiptBuilder::new_receipt_builder(state, &transaction)?;

    let execution_result = ExecutionResult::Success {
        reason: SuccessReason::Stop,
        gas_used: GAS_USED,
        gas_refunded: 0,
        logs: vec![
            ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::new()),
            ExecutionLog::new_unchecked(Address::random(), Vec::new(), Bytes::new()),
        ],
        output: Output::Call(Bytes::new()),
    };

    let execution_receipt = receipt_builder.build_receipt(
        &header,
        &transaction,
        &execution_result,
        blockchain.hardfork(),
    );

    let transaction_receipt = TransactionReceipt::new(
        execution_receipt,
        &transaction,
        &execution_result,
        0,
        0,
        blockchain.hardfork(),
    );

    let block = EthLocalBlockForChainSpec::<L1ChainSpec>::new::<L1ChainSpec>(
        &(),
        blockchain.hardfork(),
        header,
        vec![transaction],
        vec![transaction_receipt.clone()],
        Vec::new(),
        Some(Vec::new()),
    );
    let block = blockchain.insert_block(block, StateDiff::default())?;
    assert_eq!(block.block.transactions().len(), 1);

    Ok(DummyBlockAndTransaction {
        block: block.block,
        transaction_hash,
        transaction_receipt,
    })
}
