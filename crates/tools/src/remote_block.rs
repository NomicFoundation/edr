use core::fmt::Debug;
use std::sync::Arc;

use clap::ValueEnum;
use edr_eth::{
    block,
    l1::{self, L1ChainSpec},
    log::FilterLog,
    receipt::AsExecutionReceipt,
    transaction::TransactionValidation,
};
use edr_evm::{blockchain::BlockchainErrorForChainSpec, test_utils::run_full_block, BlockReceipts};
use edr_op::OpChainSpec;
use edr_provider::spec::SyncRuntimeSpec;
use edr_rpc_eth::client::EthRpcClient;

#[derive(Clone, ValueEnum)]
pub enum SupportedChainTypes {
    L1,
    Op,
}

pub async fn replay(
    chain_type: SupportedChainTypes,
    url: String,
    block_number: Option<u64>,
) -> anyhow::Result<()> {
    match chain_type {
        SupportedChainTypes::L1 => {
            replay_chain_specific_block::<L1ChainSpec>("L1", url, block_number).await
        }
        SupportedChainTypes::Op => {
            replay_chain_specific_block::<OpChainSpec>(edr_op::CHAIN_TYPE, url, block_number).await
        }
    }
}

pub async fn replay_chain_specific_block<ChainSpecT>(
    chain_type: &str,
    url: String,
    block_number: Option<u64>,
) -> anyhow::Result<()>
where
    ChainSpecT: Debug
        + SyncRuntimeSpec<
            BlockEnv: Default,
            BlockReceipt: AsExecutionReceipt<
                ExecutionReceipt = ChainSpecT::ExecutionReceipt<FilterLog>,
            >,
            ExecutionReceipt<FilterLog>: PartialEq,
            LocalBlock: BlockReceipts<
                Arc<ChainSpecT::BlockReceipt>,
                Error = BlockchainErrorForChainSpec<ChainSpecT>,
            >,
            SignedTransaction: Default
                                   + TransactionValidation<
                ValidationError: From<l1::InvalidTransaction> + Send + Sync,
            >,
        >,
{
    let rpc_client = EthRpcClient::<ChainSpecT>::new(&url, edr_defaults::CACHE_DIR.into(), None)?;

    let block_number = if let Some(block_number) = block_number {
        block_number
    } else {
        rpc_client
            .block_number()
            .await
            .map(|block_number| block_number - 20)?
    };

    println!("Testing block {block_number} for chain type {chain_type}");
    run_full_block::<ChainSpecT>(url, block_number, header_overrides).await
}

fn header_overrides(replay_header: &block::Header) -> block::HeaderOverrides {
    block::HeaderOverrides {
        beneficiary: Some(replay_header.beneficiary),
        gas_limit: Some(replay_header.gas_limit),
        extra_data: Some(replay_header.extra_data.clone()),
        mix_hash: Some(replay_header.mix_hash),
        nonce: Some(replay_header.nonce),
        parent_beacon_block_root: replay_header.parent_beacon_block_root,
        state_root: Some(replay_header.state_root),
        timestamp: Some(replay_header.timestamp),
        ..block::HeaderOverrides::default()
    }
}
