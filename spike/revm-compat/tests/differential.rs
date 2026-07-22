//! Differential test: op-revm executing over a native revm@38 `CacheDB` vs.
//! over a revm@41 `CacheDB` through `DbBridge`, with identical pre-state and
//! transactions. Execution results and committed post-state must match.
//!
//! This is the Phase 0 go/no-go evidence for the compat-layer plan.

use op_revm::{
    transaction::deposit::{DepositTransactionParts, DEPOSIT_TRANSACTION_TYPE},
    DefaultOp, OpBuilder, OpSpecId, OpTransaction,
};
use revm38::{
    context::{result::ExecutionResult as ExecutionResult38, TxEnv as TxEnv38},
    database::CacheDB as CacheDB38,
    database_interface::EmptyDB as EmptyDB38,
    state::EvmState as EvmState38,
    Context as Context38, DatabaseCommit as _, ExecuteEvm as _,
};
// Leaf primitives are the same types on both sides (shared alloy-primitives).
use revm41::primitives::{Address, Bytes, TxKind, B256, KECCAK_EMPTY, U256};
use revm41::{
    context::{BlockEnv as BlockEnv41, CfgEnv as CfgEnv41, TxEnv as TxEnv41},
    database::CacheDB as CacheDB41,
    database_interface::EmptyDB as EmptyDB41,
    DatabaseCommit as _,
};
use revm_compat_spike::{convert, db_bridge::DbBridge, hardfork::OpHardfork};

const CHAIN_ID: u64 = 10;
const ALICE: Address = Address::new([0xaa; 20]);
const BOB: Address = Address::new([0xbb; 20]);
const CHARLIE: Address = Address::new([0xcc; 20]);
const CONTRACT: Address = Address::new([0xc0; 20]);
const COINBASE: Address = Address::new([0xfe; 20]);

/// Runtime code exercising SSTORE, SLOAD (warm + cold pre-seeded slot):
///   PUSH1 42 PUSH1 0 SSTORE          → slot0 = 42
///   PUSH1 0 SLOAD PUSH1 1 ADD
///   PUSH1 1 SSTORE                   → slot1 = slot0 + 1 = 43
///   PUSH1 5 SLOAD PUSH1 6 SSTORE     → slot6 = slot5 (pre-seeded 7)
///   STOP
const CODE: &[u8] = &[
    0x60, 0x2a, 0x60, 0x00, 0x55, // slot0 = 42
    0x60, 0x00, 0x54, 0x60, 0x01, 0x01, 0x60, 0x01, 0x55, // slot1 = slot0 + 1
    0x60, 0x05, 0x54, 0x60, 0x06, 0x55, // slot6 = slot5
    0x00, // STOP
];

fn eth(amount: u64) -> U256 {
    U256::from(amount) * U256::from(10u64).pow(U256::from(18))
}

// ---------------------------------------------------------------------------
// Pre-state, seeded identically on both sides
// ---------------------------------------------------------------------------

fn seed_old(db: &mut CacheDB38<EmptyDB38>) {
    let code = revm38::bytecode::Bytecode::new_raw(Bytes::from_static(CODE));
    let code_hash = code.hash_slow();

    db.insert_account_info(
        ALICE,
        revm38::state::AccountInfo {
            balance: eth(10),
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            account_id: None,
            code: None,
        },
    );
    db.insert_account_info(
        CONTRACT,
        revm38::state::AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash,
            account_id: None,
            code: Some(code),
        },
    );
    db.insert_account_storage(CONTRACT, U256::from(5), U256::from(7))
        .expect("seeding storage cannot fail");
}

fn seed_new(db: &mut CacheDB41<EmptyDB41>) {
    let code = revm41::bytecode::Bytecode::new_raw(Bytes::from_static(CODE));
    let code_hash = code.hash_slow();

    db.insert_account_info(
        ALICE,
        revm41::state::AccountInfo {
            balance: eth(10),
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            account_id: None,
            code: None,
        },
    );
    db.insert_account_info(
        CONTRACT,
        revm41::state::AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash,
            account_id: None,
            code: Some(code),
        },
    );
    db.insert_account_storage(CONTRACT, U256::from(5), U256::from(7))
        .expect("seeding storage cannot fail");
}

// ---------------------------------------------------------------------------
// Environment and transactions, built as revm@41 values (production shape)
// and converted inbound
// ---------------------------------------------------------------------------

fn block_env_41() -> BlockEnv41 {
    BlockEnv41 {
        number: U256::from(1),
        beneficiary: COINBASE,
        timestamp: U256::from(1_700_000_000u64),
        gas_limit: 30_000_000,
        basefee: 0,
        prevrandao: Some(B256::repeat_byte(0x42)),
        ..Default::default()
    }
}

fn cfg_env_41() -> CfgEnv41<OpHardfork> {
    let mut cfg = CfgEnv41::default();
    cfg.spec = OpHardfork(OpSpecId::ISTHMUS);
    cfg.chain_id = CHAIN_ID;
    cfg
}

fn base_tx_41(caller: Address, nonce: u64, to: Address, value: U256) -> TxEnv41 {
    TxEnv41 {
        tx_type: 2,
        caller,
        gas_limit: 100_000,
        gas_price: 0,
        gas_priority_fee: Some(0),
        kind: TxKind::Call(to),
        value,
        nonce,
        chain_id: Some(CHAIN_ID),
        ..Default::default()
    }
}

fn op_tx(base_41: TxEnv41) -> OpTransaction<TxEnv38> {
    OpTransaction {
        base: convert::tx_env_new_to_old(base_41),
        // Used for the L1 data fee, which is zero here (default L1BlockInfo).
        enveloped_tx: Some(Bytes::from_static(&[0xfa; 8])),
        deposit: DepositTransactionParts::default(),
    }
}

fn tx_transfer() -> OpTransaction<TxEnv38> {
    op_tx(base_tx_41(ALICE, 0, BOB, eth(1)))
}

fn tx_contract_call() -> OpTransaction<TxEnv38> {
    op_tx(base_tx_41(ALICE, 1, CONTRACT, U256::ZERO))
}

/// Deposit with mint: exercises the op-specific execution path.
fn tx_deposit() -> OpTransaction<TxEnv38> {
    let mut base = base_tx_41(CHARLIE, 0, BOB, eth(1));
    base.tx_type = DEPOSIT_TRANSACTION_TYPE;
    base.gas_priority_fee = None;

    OpTransaction {
        base: convert::tx_env_new_to_old(base),
        enveloped_tx: Some(Bytes::from_static(&[0xfa; 8])),
        deposit: DepositTransactionParts::new(B256::repeat_byte(0x99), Some(eth(2).to()), false),
    }
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

type OpResult = ExecutionResult38<op_revm::OpHaltReason>;

/// Runs one transaction through op-revm (revm@38) over any revm@38 database,
/// mirroring the shape of `OpChainSpec::dry_run` in EDR.
fn run_tx<DatabaseT>(db: DatabaseT, tx: OpTransaction<TxEnv38>) -> (OpResult, EvmState38)
where
    DatabaseT: revm38::Database,
    DatabaseT::Error: core::fmt::Debug,
{
    let ctx = Context38::op()
        .with_db(db)
        .with_block(convert::block_env_new_to_old(block_env_41()))
        .with_cfg(convert::cfg_env_new_to_old(cfg_env_41(), |hardfork| {
            hardfork.0
        }));

    let mut evm = ctx.build_op();
    let output = evm.transact(tx).expect("transaction must execute");
    (output.result, output.state)
}

// ---------------------------------------------------------------------------
// Post-state observation through shared leaf types
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
struct AccountSnapshot {
    balance: U256,
    nonce: u64,
    code_hash: B256,
    code: Option<Bytes>,
    storage: Vec<(U256, U256)>,
}

fn snapshot_old(
    db: &mut CacheDB38<EmptyDB38>,
    address: Address,
    slots: &[u64],
) -> Option<AccountSnapshot> {
    use revm38::Database as _;

    let info = db.basic(address).expect("cache db cannot fail")?;
    let storage = slots
        .iter()
        .map(|slot| {
            let key = U256::from(*slot);
            let value = db.storage(address, key).expect("cache db cannot fail");
            (key, value)
        })
        .collect();

    Some(AccountSnapshot {
        balance: info.balance,
        nonce: info.nonce,
        code_hash: info.code_hash,
        code: info.code.map(|code| code.original_bytes()),
        storage,
    })
}

fn snapshot_new(
    db: &mut CacheDB41<EmptyDB41>,
    address: Address,
    slots: &[u64],
) -> Option<AccountSnapshot> {
    use revm41::Database as _;

    let info = db.basic(address).expect("cache db cannot fail")?;
    let storage = slots
        .iter()
        .map(|slot| {
            let key = U256::from(*slot);
            let value = db.storage(address, key).expect("cache db cannot fail");
            (key, value)
        })
        .collect();

    Some(AccountSnapshot {
        balance: info.balance,
        nonce: info.nonce,
        code_hash: info.code_hash,
        code: info.code.map(|code| code.original_bytes()),
        storage,
    })
}

// ---------------------------------------------------------------------------
// The differential test
// ---------------------------------------------------------------------------

#[test]
fn bridged_execution_matches_native() {
    let mut db_old = CacheDB38::new(EmptyDB38::default());
    seed_old(&mut db_old);

    let mut db_new = CacheDB41::new(EmptyDB41::default());
    seed_new(&mut db_new);

    let transactions = [tx_transfer(), tx_contract_call(), tx_deposit()];

    for (index, tx) in transactions.into_iter().enumerate() {
        // Native: op-revm straight on the revm@38 database.
        let (native_result, native_state) = run_tx(&mut db_old, tx.clone());

        // Bridged: identical execution over the revm@41 database, exactly as
        // EDR would run it post-upgrade.
        let (bridged_result, bridged_state) = run_tx(DbBridge::new(&mut db_new), tx);

        // Go/no-go: the bridge must not alter execution semantics.
        assert_eq!(
            native_result, bridged_result,
            "tx {index}: execution results diverged"
        );
        assert_eq!(
            native_state, bridged_state,
            "tx {index}: produced state diffs diverged"
        );
        assert!(
            native_result.is_success(),
            "tx {index}: expected success, got {native_result:?}"
        );

        // Commit each side natively; the bridged state crosses the boundary
        // through the outbound conversion (production shape).
        db_old.commit(native_state);
        db_new.commit(convert::state_old_to_new(bridged_state));

        // Outbound result conversion must preserve the observable facts.
        let converted = convert::execution_result_old_to_new(native_result.clone());
        assert_eq!(converted.tx_gas_used(), native_result.tx_gas_used());
        assert_eq!(converted.logs(), native_result.logs());
        assert_eq!(converted.is_success(), native_result.is_success());
    }

    // Post-state must be identical through both databases.
    for (address, slots) in [
        (ALICE, &[][..]),
        (BOB, &[][..]),
        (CHARLIE, &[][..]),
        (CONTRACT, &[0u64, 1, 5, 6][..]),
        (COINBASE, &[][..]),
    ] {
        let old_snapshot = snapshot_old(&mut db_old, address, slots);
        let new_snapshot = snapshot_new(&mut db_new, address, slots);
        assert_eq!(
            old_snapshot, new_snapshot,
            "post-state diverged for {address}"
        );
    }

    // Sanity: the run actually did what the transactions describe.
    let alice = snapshot_old(&mut db_old, ALICE, &[]).expect("alice exists");
    assert_eq!(alice.balance, eth(9));
    assert_eq!(alice.nonce, 2);

    let bob = snapshot_old(&mut db_old, BOB, &[]).expect("bob exists");
    assert_eq!(bob.balance, eth(2));

    let charlie = snapshot_old(&mut db_old, CHARLIE, &[]).expect("charlie exists");
    assert_eq!(charlie.balance, eth(1), "mint(2) - transfer(1)");
    assert_eq!(charlie.nonce, 1);

    let contract = snapshot_old(&mut db_old, CONTRACT, &[0, 1, 5, 6]).expect("contract exists");
    assert_eq!(
        contract.storage,
        vec![
            (U256::from(0), U256::from(42)),
            (U256::from(1), U256::from(43)),
            (U256::from(5), U256::from(7)),
            (U256::from(6), U256::from(7)),
        ]
    );
}
