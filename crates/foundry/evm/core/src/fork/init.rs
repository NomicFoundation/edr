use alloy_consensus::BlockHeader;
use alloy_primitives::{Address, U256};
use alloy_provider::{network::BlockResponse, Network, Provider};
use alloy_rpc_types::BlockNumberOrTag;
use eyre::WrapErr;
use revm::{context::CfgEnv, context_interface::Block};

use crate::{
    constants::NON_ARCHIVE_NODE_WARNING,
    evm_context::{BlockEnvMut, EvmEnv},
    opts::{BlockEnvOpts, TxEnvOpts},
    utils::apply_chain_and_block_specific_env_changes,
    AsEnvMut,
};

/// Initializes a REVM block environment based on a forked
/// ethereum provider.
pub async fn environment<NetworkT, ProviderT, BlockT, TxT, HardforkT>(
    provider: &ProviderT,
    memory_limit: u64,
    gas_price: Option<u128>,
    override_chain_id: Option<u64>,
    pin_block: Option<u64>,
    origin: Address,
    disable_block_gas_limit: bool,
) -> eyre::Result<(
    EvmEnv<BlockT, TxT, HardforkT>,
    <NetworkT as Network>::BlockResponse,
)>
where
    NetworkT: Network,
    ProviderT: Provider<NetworkT>,
    BlockT: From<BlockEnvOpts> + Block + BlockEnvMut,
    TxT: From<TxEnvOpts>,
    HardforkT: Default,
{
    let block_number = if let Some(pin_block) = pin_block {
        pin_block
    } else {
        provider
            .get_block_number()
            .await
            .wrap_err("failed to get latest block number")?
    };
    let (fork_gas_price, rpc_chain_id, block) = tokio::try_join!(
        provider.get_gas_price(),
        provider.get_chain_id(),
        provider.get_block_by_number(BlockNumberOrTag::Number(block_number))
    )?;
    let block = if let Some(block) = block {
        block
    } else {
        if let Ok(latest_block) = provider.get_block_number().await {
            // If the `eth_getBlockByNumber` call succeeds, but returns null instead of
            // the block, and the block number is less than equal the latest block, then
            // the user is forking from a non-archive node with an older block number.
            if block_number <= latest_block {
                error!("{NON_ARCHIVE_NODE_WARNING}");
            }
            eyre::bail!(
                "failed to get block for block number: {block_number}; \
                 latest block number: {latest_block}"
            );
        }
        eyre::bail!("failed to get block for block number: {block_number}")
    };

    let cfg = configure_env(
        override_chain_id.unwrap_or(rpc_chain_id),
        memory_limit,
        disable_block_gas_limit,
    );

    let mut env = EvmEnv {
        cfg,
        block: BlockEnvOpts {
            number: U256::from(block.header().number()),
            timestamp: U256::from(block.header().timestamp()),
            beneficiary: block.header().beneficiary(),
            difficulty: block.header().difficulty(),
            prevrandao: block.header().mix_hash(),
            basefee: block.header().base_fee_per_gas().unwrap_or_default(),
            gas_limit: block.header().gas_limit(),
        }
        .into(),
        tx: TxEnvOpts {
            caller: origin,
            gas_price: gas_price.unwrap_or(fork_gas_price),
            chain_id: Some(override_chain_id.unwrap_or(rpc_chain_id)),
            gas_limit: block.header().gas_limit() as u64,
        }
        .into(),
    };

    apply_chain_and_block_specific_env_changes::<NetworkT, BlockT, TxT, HardforkT>(
        env.as_env_mut(),
        &block,
    );

    Ok((env, block))
}

/// Configures the environment for the given chain id and memory limit.
pub fn configure_env<HardforkT>(
    chain_id: u64,
    memory_limit: u64,
    disable_block_gas_limit: bool,
) -> CfgEnv<HardforkT>
where
    HardforkT: Default,
{
    let mut cfg = CfgEnv::default();
    cfg.chain_id = chain_id;
    cfg.memory_limit = memory_limit;
    cfg.limit_contract_code_size = Some(usize::MAX);
    // EIP-3607 rejects transactions from senders with deployed code.
    // If EIP-3607 is enabled it can cause issues during fuzz/invariant tests if the
    // caller is a contract. So we disable the check by default.
    cfg.disable_eip3607 = true;
    cfg.disable_block_gas_limit = disable_block_gas_limit;
    cfg.disable_nonce_check = true;
    cfg
}
