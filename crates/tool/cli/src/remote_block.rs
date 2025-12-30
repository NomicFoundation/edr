use core::fmt::Debug;

use clap::ValueEnum;
use edr_block_api::EthBlockData;
use edr_block_header::{BlockHeader, HeaderOverrides};
use edr_chain_l1::L1ChainSpec;
use edr_chain_spec_provider::SyncProviderChainSpec;
use edr_chain_spec_rpc::RpcChainSpec;
use edr_op::{test_utils::jovian_header_overrides, OpChainSpec};
use edr_provider::test_utils::l1_header_overrides;
use edr_receipt::{log::FilterLog, AsExecutionReceipt};
use edr_rpc_eth::client::EthRpcClientForChainSpec;
use edr_test_block_replay::run_full_block;

#[derive(Clone, ValueEnum)]
pub enum SupportedChainTypes {
    L1,
    Op,
}

pub async fn replay(
    runtime: tokio::runtime::Handle,
    chain_type: SupportedChainTypes,
    url: String,
    block_number: Option<u64>,
) -> anyhow::Result<()> {
    match chain_type {
        SupportedChainTypes::L1 => {
            replay_chain_specific_block::<L1ChainSpec>(
                runtime,
                edr_chain_l1::CHAIN_TYPE,
                url,
                l1_header_overrides,
                block_number,
            )
            .await
        }
        SupportedChainTypes::Op => {
            replay_chain_specific_block::<OpChainSpec>(
                runtime,
                edr_op::CHAIN_TYPE,
                url,
                jovian_header_overrides,
                block_number,
            )
            .await
        }
    }
}

pub async fn replay_chain_specific_block<ChainSpecT>(
    runtime: tokio::runtime::Handle,
    chain_type: &str,
    url: String,
    header_overrides_constructor: impl FnOnce(&BlockHeader) -> HeaderOverrides<ChainSpecT::Hardfork>,
    block_number: Option<u64>,
) -> anyhow::Result<()>
where
    ChainSpecT: 'static
        + SyncProviderChainSpec<
            ExecutionReceipt<FilterLog>: Debug + PartialEq,
            Receipt: AsExecutionReceipt<ExecutionReceipt = ChainSpecT::ExecutionReceipt<FilterLog>>,
            RpcBlock<<ChainSpecT as RpcChainSpec>::RpcTransaction>: TryInto<
                EthBlockData<ChainSpecT::SignedTransaction>,
                Error: 'static,
            >,
        >,
{
    let rpc_client =
        EthRpcClientForChainSpec::<ChainSpecT>::new(&url, edr_defaults::CACHE_DIR.into(), None)?;

    let block_number = if let Some(block_number) = block_number {
        block_number
    } else {
        rpc_client
            .block_number()
            .await
            .map(|block_number| block_number - 20)?
    };

    println!("Testing block {block_number} for chain type {chain_type}");
    run_full_block::<ChainSpecT>(runtime, url, block_number, header_overrides_constructor).await
}
