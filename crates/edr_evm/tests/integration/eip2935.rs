use std::collections::BTreeMap;

use edr_chain_l1::L1ChainSpec;
use edr_eth::{block::BlockChainCondition, Bytecode};
use edr_evm::{
    blockchain::{Blockchain, LocalBlockchain},
    eips::eip2935::{
        add_history_storage_contract_to_state_diff, HISTORY_STORAGE_ADDRESS,
        HISTORY_STORAGE_UNSUPPORTED_BYTECODE,
    },
    spec::{GenesisBlockFactory as _, RuntimeSpec},
    state::StateDiff,
    GenesisBlockOptions, RandomHashGenerator,
};

fn local_blockchain(genesis_diff: StateDiff) -> anyhow::Result<LocalBlockchain<L1ChainSpec>> {
    let chain_id = 0x7a69;
    let mut prev_randao_generator = RandomHashGenerator::with_seed(edr_defaults::MIX_HASH_SEED);

    let genesis_block = L1ChainSpec::genesis_block(
        genesis_diff.clone(),
        BlockChainCondition::new(
            edr_chain_l1::Hardfork::PRAGUE,
            edr_chain_l1::L1ChainSpec::chain_base_fee_params(chain_id),
        ),
        GenesisBlockOptions {
            mix_hash: Some(prev_randao_generator.generate_next()),
            ..GenesisBlockOptions::default()
        },
    )?;

    let blockchain = LocalBlockchain::new(
        genesis_block,
        genesis_diff,
        chain_id,
        edr_chain_l1::Hardfork::PRAGUE,
    )?;

    Ok(blockchain)
}

#[test]
fn local_blockchain_without_history() -> anyhow::Result<()> {
    let pre_prague = local_blockchain(StateDiff::default())?;

    let state = pre_prague.state_at_block_number(0, &BTreeMap::default())?;

    let history_storage_account = state.basic(HISTORY_STORAGE_ADDRESS)?;
    assert!(history_storage_account.is_none());

    Ok(())
}

#[test]
fn local_blockchain_with_history() -> anyhow::Result<()> {
    // Add the history storage contract to the state diff.
    let mut state_diff = StateDiff::default();
    add_history_storage_contract_to_state_diff(&mut state_diff);

    let post_prague = local_blockchain(state_diff)?;

    let state = post_prague.state_at_block_number(0, &BTreeMap::default())?;

    let history_storage_account = state
        .basic(HISTORY_STORAGE_ADDRESS)?
        .expect("Account should exist");

    let history_storage_code = history_storage_account
        .code
        .map_or_else(|| state.code_by_hash(history_storage_account.code_hash), Ok)?;

    assert_eq!(
        history_storage_code,
        Bytecode::new_raw(HISTORY_STORAGE_UNSUPPORTED_BYTECODE)
    );

    Ok(())
}

#[cfg(feature = "test-remote")]
mod remote {
    use std::sync::Arc;

    use edr_eth::{bytes, Bytes, HashMap};
    use edr_evm::{
        blockchain::{ForkedBlockchain, ForkedCreationError},
        state::IrregularState,
    };
    use edr_rpc_eth::client::EthRpcClient;
    use edr_test_utils::env::get_alchemy_url;
    use parking_lot::Mutex;

    use super::*;

    const HISTORY_STORAGE_BYTECODE: Bytes = bytes!(
        "0x3373fffffffffffffffffffffffffffffffffffffffe14604657602036036042575f35600143038111604257611fff81430311604257611fff9006545f5260205ff35b5f5ffd5b5f35611fff60014303065500"
    );

    async fn forked_blockchain(
        irregular_state: &mut IrregularState,
        block_number: u64,
        local_hardfork: edr_chain_l1::Hardfork,
    ) -> Result<ForkedBlockchain<L1ChainSpec>, ForkedCreationError<edr_chain_l1::Hardfork>> {
        let runtime = tokio::runtime::Handle::current();

        let rpc_client = EthRpcClient::<L1ChainSpec>::new(
            &get_alchemy_url(),
            edr_defaults::CACHE_DIR.into(),
            None,
        )
        .expect("url ok");

        ForkedBlockchain::new(
            runtime,
            Some(0x7a69),
            local_hardfork,
            Arc::new(rpc_client),
            Some(block_number),
            irregular_state,
            Arc::new(Mutex::new(RandomHashGenerator::with_seed(
                edr_defaults::STATE_ROOT_HASH_SEED,
            ))),
            &HashMap::default(),
        )
        .await
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial_test::serial]
    async fn forked_blockchain_pre_prague_activation_with_cancun() -> anyhow::Result<()> {
        use edr_eth::account::AccountInfo;

        const PRE_PRAGUE_BLOCK_NUMBER: u64 = 19_426_589;

        let mut irregular_state = IrregularState::default();
        let pre_prague = forked_blockchain(
            &mut irregular_state,
            PRE_PRAGUE_BLOCK_NUMBER,
            edr_chain_l1::Hardfork::CANCUN,
        )
        .await?;

        let state = pre_prague
            .state_at_block_number(PRE_PRAGUE_BLOCK_NUMBER, irregular_state.state_overrides())?;
        let history_storage_account = state.basic(HISTORY_STORAGE_ADDRESS)?;

        // The account is either empty or a default account
        if let Some(account) = history_storage_account {
            assert_eq!(
                account,
                AccountInfo {
                    code: None,
                    ..AccountInfo::default()
                }
            );
        }

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial_test::serial]
    async fn forked_blockchain_pre_prague_activation_with_prague() -> anyhow::Result<()> {
        const PRE_PRAGUE_BLOCK_NUMBER: u64 = 19_426_589;

        let mut irregular_state = IrregularState::default();
        let pre_prague = forked_blockchain(
            &mut irregular_state,
            PRE_PRAGUE_BLOCK_NUMBER,
            edr_chain_l1::Hardfork::PRAGUE,
        )
        .await?;

        let state = pre_prague
            .state_at_block_number(PRE_PRAGUE_BLOCK_NUMBER, irregular_state.state_overrides())?;
        let history_storage_account = state
            .basic(HISTORY_STORAGE_ADDRESS)?
            .expect("Account should exist");

        let history_storage_code = history_storage_account
            .code
            .map_or_else(|| state.code_by_hash(history_storage_account.code_hash), Ok)?;

        assert_eq!(
            history_storage_code,
            Bytecode::new_raw(HISTORY_STORAGE_UNSUPPORTED_BYTECODE)
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial_test::serial]
    async fn forked_blockchain_post_eip2935_deployment_with_cancun() -> anyhow::Result<()> {
        const POST_DEPLOYMENT_BLOCK_NUMBER: u64 = 21_890_520;

        let mut irregular_state = IrregularState::default();
        let post_prague = forked_blockchain(
            &mut irregular_state,
            POST_DEPLOYMENT_BLOCK_NUMBER,
            edr_chain_l1::Hardfork::CANCUN,
        )
        .await?;

        let state = post_prague.state_at_block_number(
            POST_DEPLOYMENT_BLOCK_NUMBER,
            irregular_state.state_overrides(),
        )?;

        let history_storage_account = state
            .basic(HISTORY_STORAGE_ADDRESS)?
            .expect("Account should exist");

        let history_storage_code = history_storage_account
            .code
            .map_or_else(|| state.code_by_hash(history_storage_account.code_hash), Ok)?;

        assert_eq!(
            history_storage_code,
            Bytecode::new_raw(HISTORY_STORAGE_BYTECODE)
        );

        Ok(())
    }

    // TODO: This test is meant to start failing once the Prague hardfork is
    // activated on mainnet. Once that happens, the bytecode should be updated to
    // `HISTORY_STORAGE_BYTECODE`.
    #[tokio::test(flavor = "multi_thread")]
    #[serial_test::serial]
    async fn forked_blockchain_post_prague() -> anyhow::Result<()> {
        const POST_PRAGUE_BLOCK_NUMBER: u64 = 21_890_520;

        let mut irregular_state = IrregularState::default();
        let post_prague = forked_blockchain(
            &mut irregular_state,
            POST_PRAGUE_BLOCK_NUMBER,
            edr_chain_l1::Hardfork::PRAGUE,
        )
        .await?;

        let state = post_prague
            .state_at_block_number(POST_PRAGUE_BLOCK_NUMBER, irregular_state.state_overrides())?;

        let history_storage_account = state
            .basic(HISTORY_STORAGE_ADDRESS)?
            .expect("Account should exist");

        let history_storage_code = history_storage_account
            .code
            .map_or_else(|| state.code_by_hash(history_storage_account.code_hash), Ok)?;

        assert_eq!(
            history_storage_code,
            // TODO: Once prague has been released, this should be updated to
            // `HISTORY_STORAGE_BYTECODE`
            Bytecode::new_raw(HISTORY_STORAGE_UNSUPPORTED_BYTECODE)
        );

        Ok(())
    }
}
