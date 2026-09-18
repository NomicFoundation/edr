#![cfg(feature = "test-utils")]

//! EIP-8037: State Creation Gas Cost Increase.
//! see <https://eips.ethereum.org/EIPS/eip-8037>
//
//! From Amsterdam onward, block gas is metered in two dimensions: execution
//! gas and state gas. The header's `gas_used` is the maximum of the two block
//! totals, receipts keep summing the per-transaction total, and a transaction
//! is admitted only if it fits the remaining gas of both dimensions:
//! `min(TX_MAX_GAS_LIMIT, tx.gas) <= execution_gas_available` and
//! `tx.gas <= state_gas_available`. The EIP-7825 cap applies to execution
//! gas only, so `tx.gas` itself may exceed it.

use std::num::NonZeroU64;

use alloy_eips::eip7825::MAX_TX_GAS_LIMIT_OSAKA;
use edr_block_api::Block as _;
use edr_block_header::HeaderOverrides;
use edr_chain_l1::{
    request::Eip155,
    rpc::{call::L1CallRequest, receipt::L1RpcTransactionReceipt, TransactionRequest},
    L1ChainSpec, L1SignedTransaction, L1TransactionRequest,
};
use edr_chain_spec::ExecutableTransaction as _;
use edr_chain_spec_evm::result::ResultGas;
use edr_primitives::{address, Address, Bytecode, Bytes, U256};
use edr_provider::{
    config::{ConfigOption, ProviderConfig},
    observability::EvmObservedData,
    test_utils::{create_test_config, ProviderTestFixture},
    AccountOverride, MethodInvocation, MineBlockResultWithMetadataForChainSpec, Provider,
    ProviderRequest,
};
use edr_receipt::ExecutionReceipt as _;
use edr_transaction::{request::TransactionRequestAndSender, TxKind};
use tokio::runtime;

use crate::common::{
    bytecode::{opcode, BytecodeBuilder},
    provider::{estimate_gas, new_provider_with_config, send_transaction, transaction_receipt},
};

/// Contract creating [`FRESH_SLOTS`] new storage slots: state gas dominates.
const SLOT_CREATOR: Address = address!("0x000000000000000000000000000000000000c0de");
const FRESH_SLOTS: u8 = 10;

/// Contract creating one new slot then cold-loading many others: execution gas
/// dominates while some state gas is still spent.
const EXEC_BURNER: Address = address!("0x0000000000000000000000000000000000000b0b");
const COLD_LOADS: u8 = 80;

/// Contract looping until out of gas: spends exactly its gas limit on execution
/// and, halting, contributes no state gas.
const GAS_LOOP: Address = address!("0x0000000000000000000000000000000000001007");

const GAS_PRICE: u128 = 10_000_000_000;

/// `PUSH1 1; PUSH1 i; SSTORE` for each fresh slot `i`.
fn slot_creator_code() -> Bytes {
    let mut code = BytecodeBuilder::default();
    for slot in 1..=FRESH_SLOTS {
        code.push1(1).push1(slot).opcode(opcode::SSTORE);
    }
    code.opcode(opcode::STOP);
    code.runtime()
}

/// One fresh-slot write followed by cold `SLOAD`s of distinct slots.
fn exec_burner_code() -> Bytes {
    let mut code = BytecodeBuilder::default();
    code.push1(1).push1(0xff).opcode(opcode::SSTORE);
    for slot in 0..COLD_LOADS {
        code.push1(slot).opcode(opcode::SLOAD).opcode(opcode::POP);
    }
    code.opcode(opcode::STOP);
    code.runtime()
}

/// `JUMPDEST; PUSH1 0; JUMP`: an infinite loop.
fn gas_loop_code() -> Bytes {
    let mut code = BytecodeBuilder::default();
    code.opcode(opcode::JUMPDEST).push1(0).opcode(opcode::JUMP);
    code.runtime()
}

/// Amsterdam fixture with automining off and the given block gas limit.
fn new_fixture(block_gas_limit: u64) -> anyhow::Result<ProviderTestFixture<L1ChainSpec>> {
    new_fixture_with(block_gas_limit, |_config| {})
}

/// [`new_fixture`] after applying `customize` to the config.
fn new_fixture_with(
    block_gas_limit: u64,
    customize: impl FnOnce(&mut ProviderConfig<edr_chain_l1::Hardfork>),
) -> anyhow::Result<ProviderTestFixture<L1ChainSpec>> {
    let mut config = create_test_config();
    config.hardfork = edr_chain_l1::Hardfork::Amsterdam;
    config.mining.auto_mine = false;
    config.mining.block_gas_limit = Some(NonZeroU64::new(block_gas_limit).expect("non-zero"));
    customize(&mut config);

    config.genesis_state.insert(
        SLOT_CREATOR,
        AccountOverride {
            code: Some(Bytecode::new_raw(slot_creator_code())),
            ..AccountOverride::default()
        },
    );
    config.genesis_state.insert(
        EXEC_BURNER,
        AccountOverride {
            code: Some(Bytecode::new_raw(exec_burner_code())),
            ..AccountOverride::default()
        },
    );
    config.genesis_state.insert(
        GAS_LOOP,
        AccountOverride {
            code: Some(Bytecode::new_raw(gas_loop_code())),
            ..AccountOverride::default()
        },
    );

    let runtime = runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .thread_name("eip8037-test")
        .build()?;

    ProviderTestFixture::new(runtime, config)
}

fn sign(
    fixture: &ProviderTestFixture<L1ChainSpec>,
    nonce: u64,
    gas_limit: u64,
    kind: TxKind,
    input: Bytes,
) -> anyhow::Result<L1SignedTransaction> {
    let request = L1TransactionRequest::Eip155(Eip155 {
        nonce,
        gas_price: GAS_PRICE,
        gas_limit,
        kind,
        value: U256::ZERO,
        input,
        chain_id: fixture.config.chain_id,
    });
    let sender = fixture.nth_local_account(0)?;

    Ok(fixture
        .provider_data
        .sign_transaction_request(TransactionRequestAndSender { request, sender })?)
}

fn call(
    fixture: &ProviderTestFixture<L1ChainSpec>,
    nonce: u64,
    gas_limit: u64,
    to: Address,
) -> anyhow::Result<L1SignedTransaction> {
    sign(fixture, nonce, gas_limit, TxKind::Call(to), Bytes::new())
}

fn transfer(
    fixture: &ProviderTestFixture<L1ChainSpec>,
    nonce: u64,
    gas_limit: u64,
) -> anyhow::Result<L1SignedTransaction> {
    call(fixture, nonce, gas_limit, Address::ZERO)
}

fn deploy(
    fixture: &ProviderTestFixture<L1ChainSpec>,
    nonce: u64,
    gas_limit: u64,
    init_code: Bytes,
) -> anyhow::Result<L1SignedTransaction> {
    sign(fixture, nonce, gas_limit, TxKind::Create, init_code)
}

type MineResult = MineBlockResultWithMetadataForChainSpec<L1ChainSpec, EvmObservedData>;

/// Queues the transactions (automining is off) and mines them into one block.
fn mine(
    fixture: &mut ProviderTestFixture<L1ChainSpec>,
    transactions: Vec<L1SignedTransaction>,
) -> anyhow::Result<MineResult> {
    for transaction in transactions {
        fixture.provider_data.send_transaction(transaction)?;
    }

    Ok(fixture
        .provider_data
        .mine_and_commit_block(HeaderOverrides::default())?)
}

/// Gas of every transaction in the block, asserting they all succeeded.
fn gas_of(result: &MineResult) -> Vec<ResultGas> {
    result
        .transaction_results
        .iter()
        .map(|transaction_result| {
            assert!(
                transaction_result.is_success(),
                "transaction should succeed: {transaction_result:?}"
            );
            *transaction_result.gas()
        })
        .collect()
}

fn block_execution_gas(gas: &[ResultGas]) -> u64 {
    gas.iter().map(ResultGas::block_regular_gas_used).sum()
}

fn block_state_gas(gas: &[ResultGas]) -> u64 {
    gas.iter().map(ResultGas::block_state_gas_used).sum()
}

const DEFAULT_BLOCK_GAS_LIMIT: u64 = 30_000_000;

/// Header `gas_used` is `max(block_execution_gas_used, block_state_gas_used)`:
/// when state gas is the bottleneck, it equals the state total alone.
#[test]
fn header_gas_used_is_state_gas_when_state_dominates() -> anyhow::Result<()> {
    let mut fixture = new_fixture(DEFAULT_BLOCK_GAS_LIMIT)?;

    let transaction = call(&fixture, 0, 2_000_000, SLOT_CREATOR)?;
    let result = mine(&mut fixture, vec![transaction])?;

    let gas = gas_of(&result);
    let execution = block_execution_gas(&gas);
    let state = block_state_gas(&gas);
    assert!(
        state > execution,
        "premise: creating {FRESH_SLOTS} slots should spend more state ({state}) than execution \
         ({execution}) gas"
    );

    assert_eq!(
        result.block.block_header().gas_used,
        state,
        "header gas_used should be the state gas total (execution: {execution})"
    );

    Ok(())
}

/// Header `gas_used` is `max(block_execution_gas_used, block_state_gas_used)`:
/// when execution gas is the bottleneck, the state gas spent is not added.
#[test]
fn header_gas_used_is_execution_gas_when_execution_dominates() -> anyhow::Result<()> {
    let mut fixture = new_fixture(DEFAULT_BLOCK_GAS_LIMIT)?;

    let transaction = call(&fixture, 0, 2_000_000, EXEC_BURNER)?;
    let result = mine(&mut fixture, vec![transaction])?;

    let gas = gas_of(&result);
    let execution = block_execution_gas(&gas);
    let state = block_state_gas(&gas);
    assert!(
        execution > state && state > 0,
        "premise: the burner should spend more execution ({execution}) than state ({state}) gas, \
         and some state gas"
    );

    assert_eq!(
        result.block.block_header().gas_used,
        execution,
        "header gas_used should be the execution gas total (state: {state})"
    );

    Ok(())
}

/// A block mixing slot creation, contract deployment and a plain transfer:
/// the two state-creating transactions contribute the state gas revm charged
/// them, the transfer contributes none, and the header takes the maximum.
#[test]
fn header_gas_used_is_max_of_dimensions_for_mixed_block() -> anyhow::Result<()> {
    let mut fixture = new_fixture(DEFAULT_BLOCK_GAS_LIMIT)?;

    // ~240 bytes of deployed code so the code deposit is charged as state gas.
    let mut runtime_code = BytecodeBuilder::default();
    for _ in 0..80 {
        runtime_code.push1(0).opcode(opcode::POP);
    }
    runtime_code.opcode(opcode::STOP);

    let transactions = vec![
        call(&fixture, 0, 2_000_000, SLOT_CREATOR)?,
        deploy(&fixture, 1, 2_000_000, runtime_code.deployable())?,
        transfer(&fixture, 2, 100_000)?,
    ];
    let result = mine(&mut fixture, transactions)?;

    let gas = gas_of(&result);
    assert_eq!(gas.len(), 3, "all transactions should be mined");
    assert!(
        gas[0].block_state_gas_used() > 0,
        "slot creation should be charged state gas"
    );
    assert!(
        gas[1].block_state_gas_used() > 0,
        "contract deployment should be charged state gas"
    );
    assert_eq!(
        gas[2].block_state_gas_used(),
        0,
        "a plain transfer should not be charged state gas"
    );

    let execution = block_execution_gas(&gas);
    let state = block_state_gas(&gas);
    assert_eq!(
        result.block.block_header().gas_used,
        execution.max(state),
        "header gas_used should be max(execution: {execution}, state: {state})"
    );

    Ok(())
}

/// Receipts keep the per-transaction total: `gas_used` is the post-refund,
/// post-floor `tx_gas_used` and `cumulative_gas_used` its running sum, so it
/// differs from the header's `gas_used` whenever state gas was spent.
#[test]
fn receipt_cumulative_gas_used_is_per_transaction_total() -> anyhow::Result<()> {
    let mut fixture = new_fixture(DEFAULT_BLOCK_GAS_LIMIT)?;

    let transactions = vec![
        call(&fixture, 0, 2_000_000, SLOT_CREATOR)?,
        transfer(&fixture, 1, 100_000)?,
    ];
    let result = mine(&mut fixture, transactions)?;

    let gas = gas_of(&result);
    assert!(
        block_state_gas(&gas) > 0,
        "premise: the block should spend state gas"
    );

    let mut cumulative = 0;
    for (transaction, gas) in result.block.transactions().iter().zip(&gas) {
        let receipt = fixture
            .provider_data
            .transaction_receipt(transaction.transaction_hash())?
            .expect("receipt should exist");

        cumulative += gas.tx_gas_used();
        assert_eq!(receipt.gas_used, gas.tx_gas_used());
        assert_eq!(receipt.cumulative_gas_used(), cumulative);
    }

    assert_ne!(
        result.block.block_header().gas_used,
        cumulative,
        "header gas_used should differ from the last cumulative_gas_used when state gas is spent"
    );

    Ok(())
}

/// The next base fee follows the header's `gas_used`: with both dimensions
/// below the target but their sum above it, `max` is below target and the
/// base fee decreases.
#[test]
fn next_base_fee_follows_header_gas_used_when_state_is_bottleneck() -> anyhow::Result<()> {
    const BLOCK_GAS_LIMIT: u64 = 2_020_000;
    const GAS_TARGET: u64 = BLOCK_GAS_LIMIT / 2;

    let mut fixture = new_fixture(BLOCK_GAS_LIMIT)?;

    let transaction = call(&fixture, 0, BLOCK_GAS_LIMIT, SLOT_CREATOR)?;
    let result = mine(&mut fixture, vec![transaction])?;

    let gas = gas_of(&result);
    let execution = block_execution_gas(&gas);
    let state = block_state_gas(&gas);
    assert!(
        execution < GAS_TARGET && state < GAS_TARGET && execution + state > GAS_TARGET,
        "premise: execution ({execution}) and state ({state}) should each be below the target \
         ({GAS_TARGET}) while their sum exceeds it"
    );

    let base_fee = result
        .block
        .block_header()
        .base_fee_per_gas
        .expect("post-London block has a base fee");
    let next_base_fee = fixture
        .provider_data
        .next_block_base_fee_per_gas()?
        .expect("post-London chain has a next base fee");

    assert!(
        next_base_fee < base_fee,
        "gas_used below target should lower the base fee: {base_fee} -> {next_base_fee}"
    );

    Ok(())
}

/// Admission checks each dimension separately: a transaction fitting the
/// remaining gas of both is admitted even if the sum of everything spent and
/// its gas limit exceeds the block gas limit.
#[test]
fn admits_transaction_fitting_both_dimensions() -> anyhow::Result<()> {
    const BLOCK_GAS_LIMIT: u64 = 1_100_000;
    const SECOND_GAS_LIMIT: u64 = 100_000;

    let mut fixture = new_fixture(BLOCK_GAS_LIMIT)?;

    let transactions = vec![
        call(&fixture, 0, BLOCK_GAS_LIMIT, SLOT_CREATOR)?,
        transfer(&fixture, 1, SECOND_GAS_LIMIT)?,
    ];
    let result = mine(&mut fixture, transactions)?;

    let gas = gas_of(&result);
    let execution = gas[0].block_regular_gas_used();
    let state = gas[0].block_state_gas_used();
    assert!(
        SECOND_GAS_LIMIT <= BLOCK_GAS_LIMIT - execution
            && SECOND_GAS_LIMIT <= BLOCK_GAS_LIMIT - state
            && SECOND_GAS_LIMIT > BLOCK_GAS_LIMIT - execution - state,
        "premise: {SECOND_GAS_LIMIT} should fit both remaining dimensions (execution used \
         {execution}, state used {state}) but not their summed remainder"
    );

    assert_eq!(
        result.block.transactions().len(),
        2,
        "the second transaction should be admitted"
    );
    assert_eq!(fixture.provider_data.pending_transactions().count(), 0);

    Ok(())
}

/// A transaction is rejected when `tx.gas` exceeds the remaining state gas,
/// even though it fits the remaining execution gas.
#[test]
fn rejects_transaction_exceeding_remaining_state_gas() -> anyhow::Result<()> {
    const BLOCK_GAS_LIMIT: u64 = 1_100_000;
    const SECOND_GAS_LIMIT: u64 = 200_000;

    let mut fixture = new_fixture(BLOCK_GAS_LIMIT)?;

    let transactions = vec![
        call(&fixture, 0, BLOCK_GAS_LIMIT, SLOT_CREATOR)?,
        transfer(&fixture, 1, SECOND_GAS_LIMIT)?,
    ];
    let result = mine(&mut fixture, transactions)?;

    let gas = gas_of(&result);
    let execution = gas[0].block_regular_gas_used();
    let state = gas[0].block_state_gas_used();
    assert!(
        SECOND_GAS_LIMIT <= BLOCK_GAS_LIMIT - execution
            && SECOND_GAS_LIMIT > BLOCK_GAS_LIMIT - state,
        "premise: {SECOND_GAS_LIMIT} should fit the remaining execution gas (used {execution}) \
         but not the remaining state gas (used {state})"
    );

    assert_eq!(
        result.block.transactions().len(),
        1,
        "the second transaction should be rejected"
    );
    assert_eq!(fixture.provider_data.pending_transactions().count(), 1);

    Ok(())
}

/// A transaction is rejected when `tx.gas` exceeds the remaining execution
/// gas, even though it fits the remaining state gas.
#[test]
fn rejects_transaction_exceeding_remaining_execution_gas() -> anyhow::Result<()> {
    const BLOCK_GAS_LIMIT: u64 = 320_000;
    const SECOND_GAS_LIMIT: u64 = 150_000;

    let mut fixture = new_fixture(BLOCK_GAS_LIMIT)?;

    let transactions = vec![
        call(&fixture, 0, BLOCK_GAS_LIMIT, EXEC_BURNER)?,
        transfer(&fixture, 1, SECOND_GAS_LIMIT)?,
    ];
    let result = mine(&mut fixture, transactions)?;

    let gas = gas_of(&result);
    let execution = gas[0].block_regular_gas_used();
    let state = gas[0].block_state_gas_used();
    assert!(
        SECOND_GAS_LIMIT > BLOCK_GAS_LIMIT - execution
            && SECOND_GAS_LIMIT <= BLOCK_GAS_LIMIT - state,
        "premise: {SECOND_GAS_LIMIT} should fit the remaining state gas (used {state}) but not \
         the remaining execution gas (used {execution})"
    );

    assert_eq!(
        result.block.transactions().len(),
        1,
        "the second transaction should be rejected"
    );
    assert_eq!(fixture.provider_data.pending_transactions().count(), 1);

    Ok(())
}

/// Only `min(TX_MAX_GAS_LIMIT, tx.gas)` is checked against the remaining
/// execution gas: a transaction whose `tx.gas` exceeds both the cap and the
/// remaining execution gas is still admitted when the capped amount fits.
#[test]
fn admits_transaction_whose_capped_execution_gas_fits() -> anyhow::Result<()> {
    const FIRST_GAS_LIMIT: u64 = 12_000_000;

    let mut fixture = new_fixture_with(DEFAULT_BLOCK_GAS_LIMIT, |config| {
        config.transaction_gas_cap = ConfigOption::Default;
    })?;

    let transactions = vec![
        call(&fixture, 0, FIRST_GAS_LIMIT, GAS_LOOP)?,
        transfer(&fixture, 1, EXCEEDS_TRANSACTION_GAS_CAP)?,
    ];
    let result = mine(&mut fixture, transactions)?;

    // The gas loop halts out of gas, so it spends its whole limit on execution.
    let first = &result.transaction_results[0];
    assert!(
        first.is_halt(),
        "premise: the gas loop should halt: {first:?}"
    );
    let execution = first.gas().block_regular_gas_used();
    let state = first.gas().block_state_gas_used();
    assert!(
        EXCEEDS_TRANSACTION_GAS_CAP > DEFAULT_BLOCK_GAS_LIMIT - execution
            && MAX_TX_GAS_LIMIT_OSAKA <= DEFAULT_BLOCK_GAS_LIMIT - execution
            && EXCEEDS_TRANSACTION_GAS_CAP <= DEFAULT_BLOCK_GAS_LIMIT - state,
        "premise: tx.gas {EXCEEDS_TRANSACTION_GAS_CAP} should exceed the remaining execution gas \
         (used {execution}) while the cap {MAX_TX_GAS_LIMIT_OSAKA} fits it, and fit the remaining \
         state gas (used {state})"
    );

    assert_eq!(
        result.block.transactions().len(),
        2,
        "the second transaction should be admitted"
    );
    assert_eq!(fixture.provider_data.pending_transactions().count(), 0);

    Ok(())
}

/// Well above the EIP-7825 cap (2^24), below the default block gas limit.
const EXCEEDS_TRANSACTION_GAS_CAP: u64 = 20_000_000;

/// Amsterdam provider with the default EIP-7825 cap.
fn new_capped_provider() -> anyhow::Result<Provider<L1ChainSpec>> {
    new_provider_with_config(|config| {
        config.hardfork = edr_chain_l1::Hardfork::Amsterdam;
        config.transaction_gas_cap = ConfigOption::Default;
    })
}

fn caller(provider: &Provider<L1ChainSpec>) -> Address {
    let response = provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::Accounts(())))
        .expect("eth_accounts should succeed");
    let accounts: Vec<Address> = response
        .deserialize_result()
        .expect("response should be addresses");
    *accounts.first().expect("provider has a local account")
}

/// From Amsterdam the EIP-7825 cap applies to execution gas only, so a
/// transaction whose `tx.gas` exceeds the cap is accepted and mined.
#[tokio::test(flavor = "multi_thread")]
async fn send_transaction_accepts_gas_above_cap_from_amsterdam() -> anyhow::Result<()> {
    let provider = new_capped_provider()?;

    let request = TransactionRequest {
        from: caller(&provider),
        to: Some(Address::ZERO),
        gas: Some(EXCEEDS_TRANSACTION_GAS_CAP),
        ..TransactionRequest::default()
    };
    let transaction_hash = send_transaction(&provider, request)?;

    assert!(
        crate::common::provider::gas_used(&provider, transaction_hash) > 0,
        "the transaction should be mined"
    );

    Ok(())
}

/// From Amsterdam `eth_call` accepts a gas limit above the EIP-7825 cap.
#[tokio::test(flavor = "multi_thread")]
async fn call_accepts_gas_above_cap_from_amsterdam() -> anyhow::Result<()> {
    let provider = new_capped_provider()?;

    let call = L1CallRequest {
        from: Some(caller(&provider)),
        to: Some(Address::ZERO),
        gas: Some(EXCEEDS_TRANSACTION_GAS_CAP),
        ..L1CallRequest::default()
    };
    provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
        call, None, None,
    )))?;

    Ok(())
}

/// From Amsterdam `eth_estimateGas` accepts a gas limit above the EIP-7825
/// cap.
#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas_accepts_gas_above_cap_from_amsterdam() -> anyhow::Result<()> {
    let provider = new_capped_provider()?;

    let call = L1CallRequest {
        from: Some(caller(&provider)),
        to: Some(Address::ZERO),
        gas: Some(EXCEEDS_TRANSACTION_GAS_CAP),
        ..L1CallRequest::default()
    };
    provider.handle_request(ProviderRequest::with_single(MethodInvocation::EstimateGas(
        call, None,
    )))?;

    Ok(())
}

/// `eth_estimateGas` covers both dimensions: state gas is drawn from `tx.gas`
/// too, so a slot-creating transaction succeeds with exactly the estimate, and
/// the estimate exceeds the pre-Amsterdam one although slot creation got
/// cheaper in execution gas.
#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas_covers_state_gas() -> anyhow::Result<()> {
    let new_provider = |hardfork| {
        new_provider_with_config(|config| {
            config.hardfork = hardfork;
            config.genesis_state.insert(
                SLOT_CREATOR,
                AccountOverride {
                    code: Some(Bytecode::new_raw(slot_creator_code())),
                    ..AccountOverride::default()
                },
            );
        })
    };

    let amsterdam = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;
    let osaka = new_provider(edr_chain_l1::Hardfork::Osaka)?;

    let send_with_gas = |gas: u64| -> anyhow::Result<L1RpcTransactionReceipt> {
        let transaction_hash = send_transaction(
            &amsterdam,
            TransactionRequest {
                from: caller(&amsterdam),
                to: Some(SLOT_CREATOR),
                gas: Some(gas),
                ..TransactionRequest::default()
            },
        )?;
        transaction_receipt(&amsterdam, transaction_hash)
    };
    let request = |provider: &Provider<L1ChainSpec>| L1CallRequest {
        from: Some(caller(provider)),
        to: Some(SLOT_CREATOR),
        ..L1CallRequest::default()
    };

    let estimate = estimate_gas(&amsterdam, request(&amsterdam));
    let pre_amsterdam_estimate = estimate_gas(&osaka, request(&osaka));

    assert!(
        estimate > pre_amsterdam_estimate,
        "the Amsterdam estimate ({estimate}) should exceed the Osaka one \
         ({pre_amsterdam_estimate}): each new slot now costs state gas on top of a \
         lower execution charge"
    );

    // The pre-Amsterdam estimate does not account for the state gas.
    let receipt = send_with_gas(pre_amsterdam_estimate)?;
    assert_eq!(
        receipt.status,
        Some(false),
        "the transaction should run out of gas with the Osaka estimate"
    );

    let receipt = send_with_gas(estimate)?;
    assert_eq!(
        receipt.status,
        Some(true),
        "the transaction should succeed with the estimated gas"
    );
    assert!(receipt.gas_used <= estimate);

    Ok(())
}
