//! Implementations of [`Testing`](spec::Group::Testing) cheatcodes.

use crate::{impl_is_pure_true, Cheatcode, Cheatcodes, CheatsCtxt, Result, Vm::{rpcUrlCall, rpcUrlsCall, rpcUrlStructsCall, sleepCall, skip_0Call, skip_1Call, getChain_0Call, getChain_1Call, Chain}};
use alloy_chains::Chain as AlloyChain;
use alloy_primitives::U256;
use alloy_sol_types::SolValue;
use foundry_evm_core::{
    backend::CheatcodeBackend,
    constants::MAGIC_SKIP,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr, TransactionErrorTrait,
    },
};
use revm::context::result::HaltReasonTr;
use std::str::FromStr;

pub(crate) mod assert;
pub(crate) mod assume;
pub(crate) mod expect;
pub(crate) mod revert_handlers;

impl_is_pure_true!(rpcUrlCall);
impl Cheatcode for rpcUrlCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { rpcAlias } = self;
        let url = state.config.rpc_endpoint(rpcAlias)?.url.abi_encode();
        Ok(url)
    }
}

impl_is_pure_true!(rpcUrlsCall);
impl Cheatcode for rpcUrlsCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self {} = self;
        state.config.rpc_urls().map(|urls| urls.abi_encode())
    }
}

impl_is_pure_true!(rpcUrlStructsCall);
impl Cheatcode for rpcUrlStructsCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self {} = self;
        state.config.rpc_urls().map(|urls| urls.abi_encode())
    }
}

impl_is_pure_true!(sleepCall);
impl Cheatcode for sleepCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        _state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { duration } = self;
        let sleep_duration = std::time::Duration::from_millis(duration.saturating_to());
        std::thread::sleep(sleep_duration);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(skip_0Call);
impl Cheatcode for skip_0Call {
    fn apply_stateful<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            '_,
            '_,
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { skipTest } = *self;
        skip_1Call { skipTest, reason: String::new() }.apply_stateful(ccx)
    }
}

impl_is_pure_true!(skip_1Call);
impl Cheatcode for skip_1Call {
    fn apply_stateful<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            '_,
            '_,
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { skipTest, reason } = self;
        if *skipTest {
            // Skip should not work if called deeper than at test level.
            // Since we're not returning the magic skip bytes, this will cause a test failure.
            ensure!(ccx.ecx.journaled_state.depth <= 1, "`skip` can only be used at test level");
            Err([MAGIC_SKIP, reason.as_bytes()].concat().into())
        } else {
            Ok(Vec::default())
        }
    }
}

impl_is_pure_true!(getChain_0Call);
impl Cheatcode for getChain_0Call {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { chainAlias } = self;
        get_chain(state, chainAlias)
    }
}

impl_is_pure_true!(getChain_1Call);
impl Cheatcode for getChain_1Call {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { chainId } = self;
        // Convert the chainId to a string and use the existing get_chain function
        let chain_id_str = chainId.to_string();
        get_chain(state, &chain_id_str)
    }
}

/// Gets chain information for the given alias.
fn get_chain<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
>(
    state: &mut Cheatcodes<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    chain_alias: &str,
) -> Result {
    // Parse the chain alias - works for both chain names and IDs
    let alloy_chain = AlloyChain::from_str(chain_alias)
        .map_err(|_e| fmt_err!("invalid chain alias: {chain_alias}"))?;
    let chain_name = alloy_chain.to_string();
    let chain_id = alloy_chain.id();

    // Check if this is an unknown chain ID by comparing the name to the chain ID
    // When a numeric ID is passed for an unknown chain, alloy_chain.to_string() will return the ID
    // So if they match, it's likely an unknown chain ID
    if chain_name == chain_id.to_string() {
        return Err(fmt_err!("invalid chain alias: {chain_alias}"));
    }

    // Try to retrieve RPC URL and chain alias from user's config in foundry.toml.
    let (rpc_url, chain_alias) = if let Some(rpc_url) =
        state.config.rpc_endpoint(&chain_name).ok().map(|e| String::from(e.url))
    {
        (rpc_url, chain_name.clone())
    } else {
        (String::new(), chain_alias.to_string())
    };

    let chain_struct = Chain {
        name: chain_name,
        chainId: U256::from(chain_id),
        chainAlias: chain_alias,
        rpcUrl: rpc_url,
    };

    Ok(chain_struct.abi_encode())
}
