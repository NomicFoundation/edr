use core::fmt::Debug;
use std::{
    convert::Infallible,
    marker::PhantomData,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use alloy_rlp::Encodable as _;
use derive_where::derive_where;
use edr_block_api::{Block, EmptyBlock, LocalBlock};
use edr_block_header::{BlockConfig, BlockHeader, HeaderOverrides, PartialHeader, Withdrawal};
use edr_chain_spec::{ChainHardfork, ChainSpec, EvmSpecId, ExecutableTransaction};
use edr_primitives::{B256, KECCAK_EMPTY};
use edr_receipt::{
    log::{ExecutionLog, FilterLog, FullBlockLog, ReceiptLog},
    MapReceiptLogs, ReceiptFactory, ReceiptTrait, TransactionReceipt,
};
use edr_state_api::{StateCommit as _, StateDebug as _, StateDiff};
use edr_state_persistent_trie::PersistentStateTrie;
use edr_trie::ordered_trie_root;
use edr_utils::types::TypeConstructor;
use itertools::izip;

use crate::{
    block::BlockReceipts,
    blockchain::BlockchainError,
    spec::{ExecutionReceiptTypeConstructorBounds, ExecutionReceiptTypeConstructorForChainSpec},
    transaction::DetailedTransaction,
    GenesisBlockOptions,
};

/// Helper type for a local Ethereum block for a given chain spec.
pub type EthLocalBlockForChainSpec<ChainSpecT> = EthLocalBlock<
    <ChainSpecT as RuntimeSpec>::RpcBlockConversionError,
    <ChainSpecT as RuntimeSpec>::BlockReceipt,
    ExecutionReceiptTypeConstructorForChainSpec<ChainSpecT>,
    <ChainSpecT as ChainHardfork>::Hardfork,
    <ChainSpecT as RuntimeSpec>::RpcReceiptConversionError,
    <ChainSpecT as ChainSpec>::SignedTransaction,
>;
