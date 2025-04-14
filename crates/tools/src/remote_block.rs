use core::fmt::Debug;
use std::sync::Arc;

use clap::ValueEnum;
use edr_eth::{
    l1::{self, L1ChainSpec},
    log::FilterLog,
    receipt::AsExecutionReceipt,
    transaction::TransactionValidation,
};
use edr_evm::{BlockReceipts, blockchain::BlockchainErrorForChainSpec, test_utils::run_full_block};
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
            replay_chain_specific_block::<OpChainSpec>(
                edr_op::CHAIN_TYPE,
                block_number,
            )
            .await
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
    run_full_block::<ChainSpecT>(url, block_number).await
}
