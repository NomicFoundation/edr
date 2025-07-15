use std::num::NonZeroU64;

use alloy_dyn_abi::TypedData;
use anyhow::Context;
use edr_chain_l1::{
    test_utils::{deploy_console_log_contract, provider, ConsoleLogTransaction},
    transaction, L1ChainSpec, L1Hardfork,
};
use edr_eth::{
    block::HeaderOverrides, filter::FilteredEvents, hex, transaction::ExecutableTransaction as _,
    Address, BlockSpec, BlockTag, B256, U256,
};
use edr_evm::{state::StateOverrides, MineOrdering};
use edr_provider::{
    test_utils::{create_test_config, one_ether, ProviderTestFixture},
    MemPoolConfig, MiningConfig, ProviderConfig, ProviderErrorForChainSpec,
};
use serde_json::json;
use tokio::runtime;

#[test]
fn test_local_account_balance() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    let account = *fixture
        .provider_data
        .accounts()
        .next()
        .expect("there are local accounts");

    let last_block_number = fixture.provider_data.last_block_number();
    let block_spec = BlockSpec::Number(last_block_number);

    let balance = fixture.provider_data.balance(account, Some(&block_spec))?;

    assert_eq!(balance, one_ether());

    Ok(())
}

#[cfg(feature = "test-remote")]
#[test]
fn test_local_account_balance_forked() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_forked(None)?;

    let account = *fixture
        .provider_data
        .accounts()
        .next()
        .expect("there are local accounts");

    let last_block_number = fixture.provider_data.last_block_number();
    let block_spec = BlockSpec::Number(last_block_number);

    let balance = fixture.provider_data.balance(account, Some(&block_spec))?;

    assert_eq!(balance, one_ether());

    Ok(())
}

#[test]
fn test_sign_transaction_request() -> anyhow::Result<()> {
    let fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    let transaction = provider::signed_dummy_transaction(&fixture, 0, None)?;
    let recovered_address = transaction.caller();

    assert!(fixture
        .provider_data
        .accounts()
        .find(|address| **address == *recovered_address)
        .is_some());

    Ok(())
}

#[test]
fn test_sign_transaction_request_impersonated_account() -> anyhow::Result<()> {
    let fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    let transaction = provider::impersonated_dummy_transaction(&fixture)?;

    assert_eq!(transaction.caller(), &fixture.impersonated_account);

    Ok(())
}

fn test_add_pending_transaction(
    fixture: &mut ProviderTestFixture<L1ChainSpec>,
    transaction: transaction::Signed,
) -> anyhow::Result<()> {
    // Auto-mining must be disabled to test pending transactions
    fixture.provider_data.set_auto_mining(false);

    let filter_id = fixture
        .provider_data
        .add_pending_transaction_filter::<false>();

    let transaction_hash = fixture
        .provider_data
        .send_transaction(transaction)?
        .transaction_hash;

    assert!(fixture
        .provider_data
        .pending_transactions()
        .find(|transaction| *transaction.transaction_hash() == transaction_hash)
        .is_some());

    match fixture
        .provider_data
        .get_filter_changes(&filter_id)
        .unwrap()
    {
        FilteredEvents::NewPendingTransactions(hashes) => {
            assert!(hashes.contains(&transaction_hash));
        }
        _ => panic!("expected pending transaction"),
    };

    assert!(fixture
        .provider_data
        .pending_transactions()
        .next()
        .is_some());

    Ok(())
}

#[test]
fn add_pending_transaction() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;
    let transaction = provider::signed_dummy_transaction(&fixture, 0, None)?;

    test_add_pending_transaction(&mut fixture, transaction)
}

#[test]
fn add_pending_transaction_from_impersonated_account() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;
    let transaction = provider::impersonated_dummy_transaction(&fixture)?;

    test_add_pending_transaction(&mut fixture, transaction)
}

#[test]
fn block_by_block_spec_earliest() -> anyhow::Result<()> {
    let fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    let block_spec = BlockSpec::Tag(BlockTag::Earliest);

    let block = fixture
        .provider_data
        .block_by_block_spec(&block_spec)?
        .context("block should exist")?;

    assert_eq!(block.header().number, 0);

    Ok(())
}

#[test]
fn block_by_block_spec_finalized_safe_latest() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    // Mine a block to make sure we're not getting the genesis block
    fixture
        .provider_data
        .mine_and_commit_block(HeaderOverrides::default())?;
    let last_block_number = fixture.provider_data.last_block_number();
    // Sanity check
    assert!(last_block_number > 0);

    let block_tags = vec![BlockTag::Finalized, BlockTag::Safe, BlockTag::Latest];
    for tag in block_tags {
        let block_spec = BlockSpec::Tag(tag);

        let block = fixture
            .provider_data
            .block_by_block_spec(&block_spec)?
            .context("block should exist")?;

        assert_eq!(block.header().number, last_block_number);
    }

    Ok(())
}

#[test]
fn block_by_block_spec_pending() -> anyhow::Result<()> {
    let fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    let block_spec = BlockSpec::Tag(BlockTag::Pending);

    let block = fixture.provider_data.block_by_block_spec(&block_spec)?;

    assert!(block.is_none());

    Ok(())
}

// Make sure executing a transaction in a pending block context doesn't panic.
#[test]
fn execute_in_block_context_pending() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    let block_spec = Some(BlockSpec::Tag(BlockTag::Pending));

    let mut value = 0;
    let _ = fixture
        .provider_data
        .execute_in_block_context(block_spec.as_ref(), |_, _, _| {
            value += 1;
            Ok::<(), ProviderErrorForChainSpec<L1ChainSpec>>(())
        })?;

    assert_eq!(value, 1);

    Ok(())
}

#[test]
fn chain_id() -> anyhow::Result<()> {
    let fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    let chain_id = fixture.provider_data.chain_id();
    assert_eq!(chain_id, fixture.config.chain_id);

    Ok(())
}

#[cfg(feature = "test-remote")]
#[test]
fn chain_id_fork_mode() -> anyhow::Result<()> {
    let fixture = ProviderTestFixture::<L1ChainSpec>::new_forked(None)?;

    let chain_id = fixture.provider_data.chain_id();
    assert_eq!(chain_id, fixture.config.chain_id);

    let chain_id_at_block = fixture
        .provider_data
        .chain_id_at_block_spec(&BlockSpec::Number(1))?;
    assert_eq!(chain_id_at_block, 1);

    Ok(())
}

#[cfg(feature = "test-remote")]
#[test]
fn fork_metadata_fork_mode() -> anyhow::Result<()> {
    let fixture = ProviderTestFixture::<L1ChainSpec>::new_forked(None)?;

    let fork_metadata = fixture
        .provider_data
        .fork_metadata()
        .expect("fork metadata should exist");
    assert_eq!(fork_metadata.chain_id, 1);

    Ok(())
}

#[test]
fn console_log_mine_block() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;
    let ConsoleLogTransaction {
        transaction,
        expected_call_data,
    } = deploy_console_log_contract(&mut fixture.provider_data)?;

    let signed_transaction = fixture
        .provider_data
        .sign_transaction_request(transaction)?;

    fixture.provider_data.set_auto_mining(false);
    fixture.provider_data.send_transaction(signed_transaction)?;
    let (block_timestamp, _) = fixture.provider_data.next_block_timestamp(None)?;
    let prevrandao = fixture.provider_data.prev_randao_generator.next_value();
    let result = fixture.provider_data.mine_block(
        ProviderData::mine_block_with_mem_pool,
        HeaderOverrides {
            timestamp: Some(block_timestamp),
            mix_hash: Some(prevrandao),
            ..HeaderOverrides::default()
        },
    )?;

    let console_log_inputs = result.console_log_inputs;
    assert_eq!(console_log_inputs.len(), 1);
    assert_eq!(console_log_inputs[0], expected_call_data);

    Ok(())
}

#[test]
fn console_log_run_call() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;
    let ConsoleLogTransaction {
        transaction,
        expected_call_data,
    } = deploy_console_log_contract(&mut fixture.provider_data)?;

    let pending_transaction = fixture
        .provider_data
        .sign_transaction_request(transaction)?;

    let result = fixture.provider_data.run_call(
        pending_transaction,
        &BlockSpec::latest(),
        &StateOverrides::default(),
    )?;

    let console_log_inputs = result.console_log_inputs;
    assert_eq!(console_log_inputs.len(), 1);
    assert_eq!(console_log_inputs[0], expected_call_data);

    Ok(())
}

#[test]
fn mine_and_commit_block_empty() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    let previous_block_number = fixture.provider_data.last_block_number();

    let result = fixture
        .provider_data
        .mine_and_commit_block(HeaderOverrides::default())?;
    assert!(result.block.transactions().is_empty());

    let current_block_number = fixture.provider_data.last_block_number();
    assert_eq!(current_block_number, previous_block_number + 1);

    let cached_state = fixture
        .provider_data
        .get_or_compute_state(result.block.header().number)?;

    let calculated_state = fixture.provider_data.blockchain.state_at_block_number(
        fixture.provider_data.last_block_number(),
        fixture.provider_data.irregular_state.state_overrides(),
    )?;

    assert_eq!(cached_state.state_root()?, calculated_state.state_root()?);

    Ok(())
}

#[test]
fn mine_and_commit_blocks_empty() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    fixture
        .provider_data
        .mine_and_commit_blocks(1_000_000_000, 1)?;

    let cached_state = fixture
        .provider_data
        .get_or_compute_state(fixture.provider_data.last_block_number())?;

    let calculated_state = fixture.provider_data.blockchain.state_at_block_number(
        fixture.provider_data.last_block_number(),
        fixture.provider_data.irregular_state.state_overrides(),
    )?;

    assert_eq!(cached_state.state_root()?, calculated_state.state_root()?);

    Ok(())
}

#[test]
fn mine_and_commit_block_single_transaction() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    // Disable auto-mining to test pending transactions
    fixture.provider_data.set_auto_mining(false);

    let transaction = provider::signed_dummy_transaction(&fixture, 0, None)?;
    let expected = *transaction.value();
    let receiver = transaction
        .kind()
        .to()
        .copied()
        .expect("Dummy transaction should have a receiver");

    fixture.provider_data.send_transaction(transaction)?;

    let result = fixture
        .provider_data
        .mine_and_commit_block(HeaderOverrides::default())?;

    assert_eq!(result.block.transactions().len(), 1);

    let balance = fixture
        .provider_data
        .balance(receiver, Some(&BlockSpec::latest()))?;

    assert_eq!(balance, expected);

    Ok(())
}

#[test]
fn mine_and_commit_block_two_transactions_different_senders() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    // Disable auto-mining to test pending transactions
    fixture.provider_data.set_auto_mining(false);

    let transaction1 = provider::signed_dummy_transaction(&fixture, 0, None)?;
    let transaction2 = provider::signed_dummy_transaction(&fixture, 1, None)?;

    let receiver = transaction1
        .kind()
        .to()
        .copied()
        .expect("Dummy transaction should have a receiver");

    let expected = transaction1.value() + transaction2.value();

    fixture.provider_data.send_transaction(transaction1)?;
    fixture.provider_data.send_transaction(transaction2)?;

    let result = fixture
        .provider_data
        .mine_and_commit_block(HeaderOverrides::default())?;

    assert_eq!(result.block.transactions().len(), 2);

    let balance = fixture
        .provider_data
        .balance(receiver, Some(&BlockSpec::latest()))?;

    assert_eq!(balance, expected);

    Ok(())
}

#[test]
fn mine_and_commit_block_two_transactions_same_sender() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    // Disable auto-mining to test pending transactions
    fixture.provider_data.set_auto_mining(false);

    let transaction1 = provider::signed_dummy_transaction(&fixture, 0, Some(0))?;
    let transaction2 = provider::signed_dummy_transaction(&fixture, 0, Some(1))?;

    let receiver = transaction1
        .kind()
        .to()
        .copied()
        .expect("Dummy transaction should have a receiver");

    let expected = transaction1.value() + transaction2.value();

    fixture.provider_data.send_transaction(transaction1)?;
    fixture.provider_data.send_transaction(transaction2)?;

    let result = fixture
        .provider_data
        .mine_and_commit_block(HeaderOverrides::default())?;

    assert_eq!(result.block.transactions().len(), 2);

    let balance = fixture
        .provider_data
        .balance(receiver, Some(&BlockSpec::latest()))?;

    assert_eq!(balance, expected);

    Ok(())
}

#[test]
fn mine_and_commit_block_removes_mined_transactions() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    // Disable auto-mining to test pending transactions
    fixture.provider_data.set_auto_mining(false);

    let transaction = provider::signed_dummy_transaction(&fixture, 0, None)?;

    fixture
        .provider_data
        .send_transaction(transaction.clone())?;

    let num_pending_transactions = fixture.provider_data.pending_transactions().count();
    assert_eq!(num_pending_transactions, 1);

    let result = fixture
        .provider_data
        .mine_and_commit_block(HeaderOverrides::default())?;

    assert_eq!(result.block.transactions().len(), 1);

    let num_pending_transactions = fixture.provider_data.pending_transactions().count();
    assert_eq!(num_pending_transactions, 0);

    Ok(())
}

#[test]
fn mine_and_commit_block_leaves_unmined_transactions() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    // Disable auto-mining to test pending transactions
    fixture.provider_data.set_auto_mining(false);

    // SAFETY: literal is non-zero
    fixture
        .provider_data
        .set_block_gas_limit(unsafe { NonZeroU64::new_unchecked(55_000) })?;

    // Actual gas usage is 21_000
    let transaction1 = provider::signed_dummy_transaction(&fixture, 0, Some(0))?;
    let transaction3 = provider::signed_dummy_transaction(&fixture, 0, Some(1))?;

    // Too expensive to mine
    let transaction2 = {
        let request = provider::dummy_transaction_request(&fixture, 1, 40_000, None)?;
        fixture.provider_data.sign_transaction_request(request)?
    };

    fixture
        .provider_data
        .send_transaction(transaction1.clone())?;
    fixture
        .provider_data
        .send_transaction(transaction2.clone())?;
    fixture
        .provider_data
        .send_transaction(transaction3.clone())?;

    let pending_transactions = fixture
        .provider_data
        .pending_transactions()
        .cloned()
        .collect::<Vec<_>>();

    assert!(pending_transactions.contains(&transaction1));
    assert!(pending_transactions.contains(&transaction2));
    assert!(pending_transactions.contains(&transaction3));

    let result = fixture
        .provider_data
        .mine_and_commit_block(HeaderOverrides::default())?;

    // Check that only the first and third transactions were mined
    assert_eq!(result.block.transactions().len(), 2);
    assert!(fixture
        .provider_data
        .transaction_receipt(transaction1.transaction_hash())?
        .is_some());
    assert!(fixture
        .provider_data
        .transaction_receipt(transaction3.transaction_hash())?
        .is_some());

    // Check that the second transaction is still pending
    let pending_transactions = fixture
        .provider_data
        .pending_transactions()
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(pending_transactions, vec![transaction2]);

    Ok(())
}

#[test]
fn mine_and_commit_block_fifo_ordering() -> anyhow::Result<()> {
    let default_config = create_test_config();
    let config = ProviderConfig {
        mining: MiningConfig {
            auto_mine: false,
            mem_pool: MemPoolConfig {
                order: MineOrdering::Fifo,
            },
            ..default_config.mining
        },
        ..default_config
    };

    let runtime = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .thread_name("provider-data-test")
        .build()?;

    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new(runtime, config)?;

    let transaction1 = provider::signed_dummy_transaction(&fixture, 0, None)?;
    let transaction2 = provider::signed_dummy_transaction(&fixture, 1, None)?;

    fixture
        .provider_data
        .send_transaction(transaction1.clone())?;
    fixture
        .provider_data
        .send_transaction(transaction2.clone())?;

    let result = fixture
        .provider_data
        .mine_and_commit_block(HeaderOverrides::default())?;

    assert_eq!(result.block.transactions().len(), 2);

    let receipt1 = fixture
        .provider_data
        .transaction_receipt(transaction1.transaction_hash())?
        .expect("receipt should exist");

    assert_eq!(receipt1.transaction_index, 0);

    let receipt2 = fixture
        .provider_data
        .transaction_receipt(transaction2.transaction_hash())?
        .expect("receipt should exist");

    assert_eq!(receipt2.transaction_index, 1);

    Ok(())
}

#[test]
fn mine_and_commit_block_correct_gas_used() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    // Disable auto-mining to test pending transactions
    fixture.provider_data.set_auto_mining(false);

    let transaction1 = provider::signed_dummy_transaction(&fixture, 0, None)?;
    let transaction2 = provider::signed_dummy_transaction(&fixture, 1, None)?;

    fixture
        .provider_data
        .send_transaction(transaction1.clone())?;
    fixture
        .provider_data
        .send_transaction(transaction2.clone())?;

    let result = fixture
        .provider_data
        .mine_and_commit_block(HeaderOverrides::default())?;

    let receipt1 = fixture
        .provider_data
        .transaction_receipt(transaction1.transaction_hash())?
        .expect("receipt should exist");
    let receipt2 = fixture
        .provider_data
        .transaction_receipt(transaction2.transaction_hash())?
        .expect("receipt should exist");

    assert_eq!(receipt1.gas_used, 21_000);
    assert_eq!(receipt2.gas_used, 21_000);
    assert_eq!(
        result.block.header().gas_used,
        receipt1.gas_used + receipt2.gas_used
    );

    Ok(())
}

#[test]
fn mine_and_commit_block_rewards_miner() -> anyhow::Result<()> {
    let default_config = create_test_config();
    let config = ProviderConfig {
        hardfork: L1Hardfork::BERLIN,
        mining: MiningConfig {
            // Disable auto-mining to test pending transactions
            auto_mine: false,
            ..default_config.mining
        },
        ..default_config
    };

    let runtime = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .thread_name("provider-data-test")
        .build()?;

    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new(runtime, config)?;

    let miner = fixture.provider_data.coinbase();
    let previous_miner_balance = fixture
        .provider_data
        .balance(miner, Some(&BlockSpec::latest()))?;

    let transaction = provider::signed_dummy_transaction(&fixture, 0, None)?;
    fixture
        .provider_data
        .send_transaction(transaction.clone())?;

    fixture
        .provider_data
        .mine_and_commit_block(HeaderOverrides::default())?;

    let miner_balance = fixture
        .provider_data
        .balance(miner, Some(&BlockSpec::latest()))?;

    assert!(miner_balance > previous_miner_balance);

    Ok(())
}

#[test]
fn mine_and_commit_blocks_increases_block_number() -> anyhow::Result<()> {
    const NUM_MINED_BLOCKS: u64 = 10;

    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    let previous_block_number = fixture.provider_data.last_block_number();

    fixture
        .provider_data
        .mine_and_commit_blocks(NUM_MINED_BLOCKS, 1)?;

    assert_eq!(
        fixture.provider_data.last_block_number(),
        previous_block_number + NUM_MINED_BLOCKS
    );
    assert_eq!(
        fixture.provider_data.last_block()?.header().number,
        previous_block_number + NUM_MINED_BLOCKS
    );

    Ok(())
}

#[test]
fn mine_and_commit_blocks_works_with_snapshots() -> anyhow::Result<()> {
    const NUM_MINED_BLOCKS: u64 = 10;

    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    // Disable auto-mining to test pending transactions
    fixture.provider_data.set_auto_mining(false);

    let transaction1 = provider::signed_dummy_transaction(&fixture, 0, None)?;
    let transaction2 = provider::signed_dummy_transaction(&fixture, 1, None)?;

    let original_block_number = fixture.provider_data.last_block_number();

    fixture
        .provider_data
        .send_transaction(transaction1.clone())?;

    let snapshot_id = fixture.provider_data.make_snapshot();
    assert_eq!(
        fixture.provider_data.last_block_number(),
        original_block_number
    );

    // Mine block after snapshot
    fixture
        .provider_data
        .mine_and_commit_blocks(NUM_MINED_BLOCKS, 1)?;

    assert_eq!(
        fixture.provider_data.last_block_number(),
        original_block_number + NUM_MINED_BLOCKS
    );

    let reverted = fixture.provider_data.revert_to_snapshot(snapshot_id);
    assert!(reverted);

    assert_eq!(
        fixture.provider_data.last_block_number(),
        original_block_number
    );

    fixture
        .provider_data
        .mine_and_commit_blocks(NUM_MINED_BLOCKS, 1)?;

    let block_number_before_snapshot = fixture.provider_data.last_block_number();

    // Mine block before snapshot
    let snapshot_id = fixture.provider_data.make_snapshot();

    fixture
        .provider_data
        .send_transaction(transaction2.clone())?;

    fixture.provider_data.mine_and_commit_blocks(1, 1)?;

    let reverted = fixture.provider_data.revert_to_snapshot(snapshot_id);
    assert!(reverted);

    assert_eq!(
        fixture.provider_data.last_block_number(),
        block_number_before_snapshot
    );

    Ok(())
}

#[test]
fn next_filter_id() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    let mut prev_filter_id = fixture.provider_data.last_filter_id;
    for _ in 0..10 {
        let filter_id = fixture.provider_data.next_filter_id();
        assert!(prev_filter_id < filter_id);
        prev_filter_id = filter_id;
    }

    Ok(())
}

#[test]
fn pending_transactions_returns_pending_and_queued() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local().unwrap();

    // Disable auto-mining to test pending transactions
    fixture.provider_data.set_auto_mining(false);

    let transaction1 = provider::signed_dummy_transaction(&fixture, 0, Some(0))?;
    fixture
        .provider_data
        .send_transaction(transaction1.clone())?;

    let transaction2 = provider::signed_dummy_transaction(&fixture, 0, Some(2))?;
    fixture
        .provider_data
        .send_transaction(transaction2.clone())?;

    let transaction3 = provider::signed_dummy_transaction(&fixture, 0, Some(3))?;
    fixture
        .provider_data
        .send_transaction(transaction3.clone())?;

    let pending_transactions = fixture
        .provider_data
        .pending_transactions()
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(
        pending_transactions,
        vec![transaction1, transaction2, transaction3]
    );

    Ok(())
}

#[test]
fn set_balance_updates_mem_pool() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    // Disable auto-mining to test pending transactions
    fixture.provider_data.set_auto_mining(false);

    let transaction = provider::impersonated_dummy_transaction(&fixture)?;
    let transaction_hash = fixture
        .provider_data
        .send_transaction(transaction)?
        .transaction_hash;

    assert!(fixture
        .provider_data
        .pending_transactions()
        .find(|transaction| *transaction.transaction_hash() == transaction_hash)
        .is_some());

    fixture
        .provider_data
        .set_balance(fixture.impersonated_account, U256::from(100))?;

    assert!(fixture
        .provider_data
        .pending_transactions()
        .find(|transaction| *transaction.transaction_hash() == transaction_hash)
        .is_none());

    Ok(())
}

#[test]
fn transaction_by_invalid_hash() -> anyhow::Result<()> {
    let fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    let non_existing_tx = fixture.provider_data.transaction_by_hash(&B256::ZERO)?;

    assert!(non_existing_tx.is_none());

    Ok(())
}

#[test]
fn pending_transaction_by_hash() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    // Disable auto-mining to test pending transactions
    fixture.provider_data.set_auto_mining(false);

    let transaction_request = provider::signed_dummy_transaction(&fixture, 0, None)?;
    let transaction_hash = fixture
        .provider_data
        .send_transaction(transaction_request)?
        .transaction_hash;

    let transaction_result = fixture
        .provider_data
        .transaction_by_hash(&transaction_hash)?
        .context("transaction not found")?;

    assert_eq!(
        transaction_result.transaction.transaction_hash(),
        &transaction_hash
    );

    Ok(())
}

#[test]
fn transaction_by_hash() -> anyhow::Result<()> {
    let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    // Disable auto-mining to test pending transactions
    fixture.provider_data.set_auto_mining(false);

    let transaction_request = provider::signed_dummy_transaction(&fixture, 0, None)?;
    let transaction_hash = fixture
        .provider_data
        .send_transaction(transaction_request)?
        .transaction_hash;

    let results = fixture
        .provider_data
        .mine_and_commit_block(HeaderOverrides::default())?;

    // Make sure transaction was mined successfully.
    assert!(results
        .transaction_results
        .first()
        .context("failed to mine transaction")?
        .is_success());

    // Sanity check that the mempool is empty.
    assert_eq!(fixture.provider_data.pending_transactions().count(), 0);

    let transaction_result = fixture
        .provider_data
        .transaction_by_hash(&transaction_hash)?
        .context("transaction not found")?;

    assert_eq!(
        transaction_result.transaction.transaction_hash(),
        &transaction_hash
    );

    Ok(())
}

#[test]
fn sign_typed_data_v4() -> anyhow::Result<()> {
    let fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

    // This test was taken from the `eth_signTypedData` example from the
    // EIP-712 specification via Hardhat.
    // <https://eips.ethereum.org/EIPS/eip-712#eth_signtypeddata>

    let address: Address = "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".parse()?;
    let message = json!({
      "types": {
        "EIP712Domain": [
          { "name": "name", "type": "string" },
          { "name": "version", "type": "string" },
          { "name": "chainId", "type": "uint256" },
          { "name": "verifyingContract", "type": "address" },
        ],
        "Person": [
          { "name": "name", "type": "string" },
          { "name": "wallet", "type": "address" },
        ],
        "Mail": [
          { "name": "from", "type": "Person" },
          { "name": "to", "type": "Person" },
          { "name": "contents", "type": "string" },
        ],
      },
      "primaryType": "Mail",
      "domain": {
        "name": "Ether Mail",
        "version": "1",
        "chainId": 1,
        "verifyingContract": "0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC",
      },
      "message": {
        "from": {
          "name": "Cow",
          "wallet": "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826",
        },
        "to": {
          "name": "Bob",
          "wallet": "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB",
        },
        "contents": "Hello, Bob!",
      },
    });
    let message: TypedData = serde_json::from_value(message)?;

    let signature = fixture
        .provider_data
        .sign_typed_data_v4(&address, &message)?;

    let expected_signature = "0x4355c47d63924e8a72e509b65029052eb6c299d53a04e167c5775fd466751c9d07299936d304c153f6443dfa05f40ff007d72911b6f72307f996231605b915621c";

    assert_eq!(hex::decode(expected_signature)?, signature.to_vec(),);

    Ok(())
}

#[cfg(feature = "test-remote")]
mod alchemy {
    use edr_chain_l1::{L1HaltReason, L1InvalidTransaction};
    use edr_eth::{block, result::ExecutionResult, HashMap};
    use edr_evm::impl_full_block_tests;
    use edr_provider::{
        hardhat_rpc_types::ResetForkConfig, test_utils::FORK_BLOCK_NUMBER, CallResult, ForkConfig,
        ProviderData, ProviderError,
    };
    use edr_test_utils::env::get_alchemy_url;

    use super::*;

    #[test]
    fn reset_local_to_forking() -> anyhow::Result<()> {
        let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;

        let fork_config = Some(ResetForkConfig {
            json_rpc_url: get_alchemy_url(),
            // Random recent block for better cache consistency
            block_number: Some(FORK_BLOCK_NUMBER),
            http_headers: None,
        });

        let block_spec = BlockSpec::Number(FORK_BLOCK_NUMBER);

        assert_eq!(fixture.provider_data.last_block_number(), 0);

        fixture.provider_data.reset(fork_config)?;

        // We're fetching a specific block instead of the last block number for the
        // forked blockchain, because the last block number query cannot be
        // cached.
        assert!(fixture
            .provider_data
            .block_by_block_spec(&block_spec)?
            .is_some());

        Ok(())
    }

    #[test]
    fn reset_forking_to_local() -> anyhow::Result<()> {
        let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_forked(None)?;

        // We're fetching a specific block instead of the last block number for the
        // forked blockchain, because the last block number query cannot be
        // cached.
        assert!(fixture
            .provider_data
            .block_by_block_spec(&BlockSpec::Number(FORK_BLOCK_NUMBER))?
            .is_some());

        fixture.provider_data.reset(None)?;

        assert_eq!(fixture.provider_data.last_block_number(), 0);

        Ok(())
    }

    #[test]
    fn run_call_in_hardfork_context() -> anyhow::Result<()> {
        use alloy_sol_types::{sol, SolCall};
        use edr_evm::transaction::TransactionError;
        use edr_provider::{
            requests::eth::resolve_call_request, test_utils::create_test_config_with_fork,
        };
        use edr_rpc_eth::CallRequest;

        sol! { function Hello() public pure returns (string); }

        fn assert_decoded_output(result: ExecutionResult<L1HaltReason>) -> anyhow::Result<()> {
            let output = result.into_output().expect("Call must have output");
            let decoded = HelloCall::abi_decode_returns(output.as_ref(), false)?;

            assert_eq!(decoded._0, "Hello World");
            Ok(())
        }

        /// Executes a call to method `Hello` on contract `HelloWorld`,
        /// deployed to mainnet.
        ///
        /// Should return a string `"Hello World"`.
        fn call_hello_world_contract(
            data: &mut ProviderData<L1ChainSpec>,
            block_spec: BlockSpec,
            request: CallRequest,
        ) -> Result<CallResult<L1HaltReason>, ProviderErrorForChainSpec<L1ChainSpec>> {
            let state_overrides = StateOverrides::default();

            let transaction = resolve_call_request(data, request, &block_spec, &state_overrides)?;

            data.run_call(transaction, &block_spec, &state_overrides)
        }

        const EIP_1559_ACTIVATION_BLOCK: u64 = 12_965_000;
        const HELLO_WORLD_CONTRACT_ADDRESS: &str = "0xe36613A299bA695aBA8D0c0011FCe95e681f6dD3";

        let hello_world_contract_address: Address = HELLO_WORLD_CONTRACT_ADDRESS.parse()?;
        let hello_world_contract_call = HelloCall::new(());

        let runtime = runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .thread_name("provider-data-test")
            .build()?;

        let default_config = create_test_config_with_fork(Some(ForkConfig {
            block_number: Some(EIP_1559_ACTIVATION_BLOCK),
            cache_dir: edr_defaults::CACHE_DIR.into(),
            chain_overrides: HashMap::new(),
            http_headers: None,
            url: get_alchemy_url(),
        }));

        let config = ProviderConfig {
            // SAFETY: literal is non-zero
            block_gas_limit: unsafe { NonZeroU64::new_unchecked(1_000_000) },
            chain_id: 1,
            coinbase: Address::ZERO,
            hardfork: L1Hardfork::LONDON,
            network_id: 1,
            ..default_config
        };

        let mut fixture = ProviderTestFixture::<L1ChainSpec>::new(runtime, config)?;

        let default_call = CallRequest {
            from: Some(fixture.nth_local_account(0)?),
            to: Some(hello_world_contract_address),
            gas: Some(1_000_000),
            value: Some(U256::ZERO),
            data: Some(hello_world_contract_call.abi_encode().into()),
            ..CallRequest::default()
        };

        // Should accept post-EIP-1559 gas semantics when running in the context of a
        // post-EIP-1559 block
        let result = call_hello_world_contract(
            &mut fixture.provider_data,
            BlockSpec::Number(EIP_1559_ACTIVATION_BLOCK),
            CallRequest {
                max_fee_per_gas: Some(0),
                ..default_call.clone()
            },
        )?;

        assert_decoded_output(result.execution_result)?;

        // Should accept pre-EIP-1559 gas semantics when running in the context of a
        // pre-EIP-1559 block
        let result = call_hello_world_contract(
            &mut fixture.provider_data,
            BlockSpec::Number(EIP_1559_ACTIVATION_BLOCK - 1),
            CallRequest {
                gas_price: Some(0),
                ..default_call.clone()
            },
        )?;

        assert_decoded_output(result.execution_result)?;

        // Should throw when given post-EIP-1559 gas semantics and when running in the
        // context of a pre-EIP-1559 block
        let result = call_hello_world_contract(
            &mut fixture.provider_data,
            BlockSpec::Number(EIP_1559_ACTIVATION_BLOCK - 1),
            CallRequest {
                max_fee_per_gas: Some(0),
                ..default_call.clone()
            },
        );

        assert!(matches!(
            result,
            Err(ProviderError::RunTransaction(
                TransactionError::InvalidTransaction(L1InvalidTransaction::Eip1559NotSupported)
            ))
        ));

        // Should accept pre-EIP-1559 gas semantics when running in the context of a
        // post-EIP-1559 block
        let result = call_hello_world_contract(
            &mut fixture.provider_data,
            BlockSpec::Number(EIP_1559_ACTIVATION_BLOCK),
            CallRequest {
                gas_price: Some(0),
                ..default_call.clone()
            },
        )?;

        assert_decoded_output(result.execution_result)?;

        // Should support a historical call in the context of a block added via
        // `mine_and_commit_blocks`
        let previous_block_number = fixture.provider_data.last_block_number();

        fixture.provider_data.mine_and_commit_blocks(100, 1)?;

        let result = call_hello_world_contract(
            &mut fixture.provider_data,
            BlockSpec::Number(previous_block_number + 50),
            CallRequest {
                max_fee_per_gas: Some(0),
                ..default_call
            },
        )?;

        assert_decoded_output(result.execution_result)?;

        Ok(())
    }

    fn l1_header_overrides(replay_header: &block::Header) -> HeaderOverrides {
        HeaderOverrides {
            beneficiary: Some(replay_header.beneficiary),
            gas_limit: Some(replay_header.gas_limit),
            extra_data: Some(replay_header.extra_data.clone()),
            mix_hash: Some(replay_header.mix_hash),
            nonce: Some(replay_header.nonce),
            parent_beacon_block_root: replay_header.parent_beacon_block_root,
            state_root: Some(replay_header.state_root),
            timestamp: Some(replay_header.timestamp),
            ..HeaderOverrides::default()
        }
    }

    impl_full_block_tests! {
        mainnet_byzantium => L1ChainSpec {
            block_number: 4_370_001,
            url: get_alchemy_url(),
            header_overrides_constructor: l1_header_overrides,
        },
        mainnet_constantinople => L1ChainSpec {
            block_number: 7_280_001,
            url: get_alchemy_url(),
            header_overrides_constructor: l1_header_overrides,
        },
        mainnet_istanbul => L1ChainSpec {
            block_number: 9_069_001,
            url: get_alchemy_url(),
            header_overrides_constructor: l1_header_overrides,
        },
        mainnet_muir_glacier => L1ChainSpec {
            block_number: 9_300_077,
            url: get_alchemy_url(),
            header_overrides_constructor: l1_header_overrides,
        },
        mainnet_shanghai => L1ChainSpec {
            block_number: 17_050_001,
            url: get_alchemy_url(),
            header_overrides_constructor: l1_header_overrides,
        },
        // This block contains a sequence of transaction that first raise
        // an empty account's balance and then decrease it
        mainnet_19318016 => L1ChainSpec {
            block_number: 19_318_016,
            url: get_alchemy_url(),
            header_overrides_constructor: l1_header_overrides,
        },
        // This block has both EIP-2930 and EIP-1559 transactions
        sepolia_eip_1559_2930 => L1ChainSpec {
            block_number: 5_632_795,
            url: get_alchemy_url().replace("mainnet", "sepolia"),
            header_overrides_constructor: l1_header_overrides,
        },
        sepolia_shanghai => L1ChainSpec {
            block_number: 3_095_000,
            url: get_alchemy_url().replace("mainnet", "sepolia"),
            header_overrides_constructor: l1_header_overrides,
        },
        // This block has an EIP-4844 transaction
        mainnet_cancun => L1ChainSpec {
            block_number: 19_529_021,
            url: get_alchemy_url(),
            header_overrides_constructor: l1_header_overrides,
        },
        // This block contains a transaction that uses the KZG point evaluation
        // precompile, introduced in Cancun
        mainnet_cancun2 => L1ChainSpec {
            block_number: 19_562_047,
            url: get_alchemy_url(),
            header_overrides_constructor: l1_header_overrides,
        },
    }
}
