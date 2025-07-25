use std::{collections::BTreeMap, str::FromStr};

use edr_coverage::CoverageHitCollector;
use edr_eth::{
    bytes,
    l1::{self, L1ChainSpec},
    result::{ExecutionResult, Output},
    signature::public_key_to_address,
    transaction::{self, TxKind},
    Address, Bytes, HashMap, HashSet, B256, U256,
};
use edr_evm::{
    blockchain::{Blockchain, GenesisBlockOptions, LocalBlockchain},
    config::CfgEnv,
    runtime::{dry_run_with_inspector, run},
    state::{AccountModifierFn, StateDiff, StateError, SyncState},
};
use edr_test_utils::secret_key::secret_key_from_str;

const CHAIN_ID: u64 = 31337;

const INCREMENT_DEPLOYED_BYTECODE: &str =
    include_str!("../../../../data/deployed_bytecode/increment.in");

fn deploy_contract(
    blockchain: &LocalBlockchain<L1ChainSpec>,
    state: &mut dyn SyncState<StateError>,
    bytecode: Bytes,
) -> anyhow::Result<Address> {
    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let caller = public_key_to_address(secret_key.public_key());

    let nonce = state.basic(caller)?.map_or(0, |info| info.nonce);
    let request = transaction::request::Eip1559 {
        chain_id: CHAIN_ID,
        nonce,
        max_priority_fee_per_gas: 1_000,
        max_fee_per_gas: 1_000,
        gas_limit: 1_000_000,
        kind: TxKind::Create,
        value: U256::ZERO,
        input: bytecode,
        access_list: Vec::new(),
    };

    let signed = request.sign(&secret_key)?;

    let cfg = CfgEnv::new_with_spec(blockchain.hardfork()).with_chain_id(blockchain.chain_id());
    let block = l1::BlockEnv {
        number: 1,
        ..l1::BlockEnv::default()
    };

    let result = run::<_, L1ChainSpec, _>(
        blockchain,
        state,
        cfg,
        signed.into(),
        block,
        &HashMap::new(),
    )?;
    let address = if let ExecutionResult::Success {
        output: Output::Create(_, Some(address)),
        ..
    } = result
    {
        address
    } else {
        panic!("Expected a contract creation, but got: {result:?}");
    };

    Ok(address)
}

fn call_inc_by(
    blockchain: &LocalBlockchain<L1ChainSpec>,
    state: &dyn SyncState<StateError>,
    deployed_address: Address,
    increment: U256,
) -> anyhow::Result<HashSet<Bytes>> {
    // > cast sig 'incBy(uint)'
    const SELECTOR: &str = "0x70119d06";

    // > cast calldata 'function incBy(uint)' 1
    // 0x70119d060000000000000000000000000000000000000000000000000000000000000001
    let encoded = format!("{SELECTOR}{increment:0>64x}");

    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let caller = public_key_to_address(secret_key.public_key());

    let nonce = state.basic(caller)?.map_or(0, |info| info.nonce);
    let request = transaction::request::Eip1559 {
        chain_id: CHAIN_ID,
        nonce,
        max_priority_fee_per_gas: 1_000,
        max_fee_per_gas: 1_000,
        gas_limit: 1_000_000,
        kind: TxKind::Call(deployed_address),
        value: U256::ZERO,
        input: Bytes::from_str(&encoded).expect("Failed to parse hex"),
        access_list: Vec::new(),
    };

    let signed = request.sign(&secret_key)?;

    let cfg = CfgEnv::new_with_spec(l1::SpecId::CANCUN).with_chain_id(blockchain.chain_id());
    let block = l1::BlockEnv {
        number: 1,
        ..l1::BlockEnv::default()
    };

    let mut coverage_collector = CoverageHitCollector::default();
    let result = dry_run_with_inspector::<_, L1ChainSpec, _, _>(
        blockchain,
        state,
        cfg,
        signed.into(),
        block,
        &HashMap::new(),
        &mut coverage_collector,
    )?;

    assert!(
        !result.result.is_halt(),
        "Expected success or revert, but got: {result:?}"
    );

    Ok(coverage_collector.into_hits())
}

#[test]
fn record_hits() -> anyhow::Result<()> {
    let blockchain = LocalBlockchain::<L1ChainSpec>::new(
        StateDiff::default(),
        CHAIN_ID,
        l1::SpecId::CANCUN,
        GenesisBlockOptions {
            mix_hash: Some(B256::random()),
            ..GenesisBlockOptions::default()
        },
    )?;

    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let caller = public_key_to_address(secret_key.public_key());

    let mut state = blockchain.state_at_block_number(0, &BTreeMap::new())?;
    state.modify_account(
        caller,
        AccountModifierFn::new(Box::new(|balance, _nonce, _code| {
            *balance = U256::from(100_000_000_000_000u128);
        })),
    )?;

    let increment = deploy_contract(
        &blockchain,
        &mut state,
        Bytes::from_str(INCREMENT_DEPLOYED_BYTECODE).expect("Invalid bytecode"),
    )
    .expect("Failed to deploy");

    // Trigger a revert after we've collected the first hit
    let hits = call_inc_by(&blockchain, &state, increment, U256::ZERO)?;
    assert_eq!(hits.len(), 1);
    assert_eq!(
        hits,
        [bytes!(
            "0x0000000000000000000000000000000000000000000000000000000000000001"
        )]
        .into_iter()
        .collect()
    );

    // Successfully execute the `incBy` function, resulting in two hits.
    let hits = call_inc_by(&blockchain, &state, increment, U256::from(1))?;
    assert_eq!(hits.len(), 2);
    assert_eq!(
        hits,
        [
            bytes!("0x0000000000000000000000000000000000000000000000000000000000000001"),
            bytes!("0x0000000000000000000000000000000000000000000000000000000000000002")
        ]
        .into_iter()
        .collect()
    );

    Ok(())
}
