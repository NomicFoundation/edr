use core::fmt::Debug;
use std::sync::Arc;

use clap::ValueEnum;
use edr_chain_l1::L1ChainSpec;
use edr_eth::block;
use edr_evm::{blockchain::BlockchainErrorForChainSpec, test_utils::run_full_block, BlockReceipts};
use edr_evm_spec::{EvmTransactionValidationError, TransactionValidation};
use edr_op::{test_utils::isthmus_header_overrides, OpChainSpec};
use edr_provider::{spec::SyncRuntimeSpec, test_utils::l1_header_overrides};
use edr_receipt::{log::FilterLog, AsExecutionReceipt};
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
            replay_chain_specific_block::<L1ChainSpec>(
                edr_chain_l1::CHAIN_TYPE,
                url,
                l1_header_overrides,
                block_number,
            )
            .await
        }
        SupportedChainTypes::Op => {
            replay_chain_specific_block::<OpChainSpec>(
                edr_op::CHAIN_TYPE,
                url,
                isthmus_header_overrides,
                block_number,
            )
            .await
        }
    }
}

pub async fn replay_chain_specific_block<ChainSpecT>(
    chain_type: &str,
    url: String,
    header_overrides_constructor: impl FnOnce(
        &block::Header,
    ) -> block::HeaderOverrides<ChainSpecT::Hardfork>,
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
                ValidationError: From<EvmTransactionValidationError> + Send + Sync,
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
    run_full_block::<ChainSpecT>(url, block_number, header_overrides_constructor).await
}
