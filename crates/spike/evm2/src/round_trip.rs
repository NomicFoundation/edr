//! Spike A: the evm2 adapter round trip. Each case runs through
//! `edr_evm::dry_run` (revm) and `adapter::dry_run` (evm2) with the same EDR
//! inputs: `CfgEnv`, EDR's block env (`HeaderAndEvmSpec`), an
//! `L1SignedTransaction` and revm `PrecompileFn`s. Results, errors, state
//! changes (as committed state roots) and call traces are compared with `==`.

use alloy_primitives::{hex, TxKind};
use edr_block_header::{BlobGas, BlockHeader, HeaderAndEvmSpec};
use edr_blockchain_api::BlockHashByNumber;
use edr_chain_l1::{L1ChainSpec, L1Hardfork, L1SignedTransaction, L1TransactionRequest};
use edr_chain_spec_evm::CfgEnv;
use edr_eip2930::AccessListItem;
use edr_eip7702::{Authorization, SignedAuthorization};
use edr_precompile::{PrecompileFn, PrecompileResult};
use edr_primitives::{address, keccak256, Address, Bytecode as RevmBytecode, Bytes, HashMap, B256, U256};
use edr_signer::{
    public_key_to_address, secret_key_from_str, FakeSign as _,
    SignatureWithYParity,
};
use edr_state_api::{account::AccountInfo as EdrAccountInfo, StateCommit as _, StateDebug as _};
use edr_state_persistent_trie::PersistentStateTrie;
use edr_transaction::request::{Eip1559, Eip155, Eip2930, Eip4844, Eip7702, Legacy};
use revm_inspectors::tracing::{CallTraceArena, TracingInspector, TracingInspectorConfig};

use crate::{adapter, Asm, CALLER, LOGGER, RECIPIENT, STORAGE};

const CHAIN_ID: u64 = 31_337;
const GWEI: u128 = 1_000_000_000;
const BLOCK_ENV: Address = address!("0x00000000000000000000000000000000000000f0");
const CODE_CALLER: Address = address!("0x00000000000000000000000000000000000000f1");
const POOR: Address = address!("0x00000000000000000000000000000000000000f2");
const NONCE_3: Address = address!("0x00000000000000000000000000000000000000f3");
const CUSTOM_PRECOMPILE: Address = address!("0x0000000000000000000000000000000000000100");
const REVERTING_PRECOMPILE: Address = address!("0x0000000000000000000000000000000000000101");
const HALTING_PRECOMPILE: Address = address!("0x0000000000000000000000000000000000000102");
const IDENTITY: Address = address!("0x0000000000000000000000000000000000000004");
// Anvil's first test key.
const AUTHORITY_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

/// Hashes for `BLOCKHASH`, the same for both engines.
struct FakeBlockHashes;

#[derive(Debug, thiserror::Error)]
#[error("unreachable")]
struct Never;

impl BlockHashByNumber for FakeBlockHashes {
    type Error = Never;

    fn block_hash_by_number(&self, block_number: u64) -> Result<B256, Self::Error> {
        Ok(keccak256(block_number.to_be_bytes()))
    }
}

/// Returns, as 32-byte words: NUMBER, TIMESTAMP, COINBASE, BASEFEE,
/// PREVRANDAO, GASLIMIT, BLOBBASEFEE, CHAINID, GASPRICE, ORIGIN, BLOBHASH(0),
/// BLOCKHASH(NUMBER - 1).
fn block_env_contract() -> Bytes {
    let mut asm = Asm::default();
    let ops: [&[u8]; 12] = [
        &[0x43],
        &[0x42],
        &[0x41],
        &[0x48],
        &[0x44],
        &[0x45],
        &[0x4a],
        &[0x46],
        &[0x3a],
        &[0x32],
        &[0x60, 0x00, 0x49],
        &[0x60, 0x01, 0x43, 0x03, 0x40],
    ];
    let count = ops.len() as u64;
    for (index, op) in ops.into_iter().enumerate() {
        for byte in op {
            asm = asm.op(*byte);
        }
        asm = asm.push_u(32 * index as u64).op(0x52); // MSTORE
    }
    asm.push_u(32 * count).push_u(0).op(0xf3).build() // RETURN
}

/// Init code deploying `size` bytes of runtime code.
fn init_deploying(size: u64) -> Bytes {
    Asm::default()
        .push_u(size)
        .push_u(0)
        .op(0xf3) // RETURN(0, size): zeroed memory
        .build()
}

#[allow(deprecated)]
fn authority_key() -> edr_signer::SecretKey {
    secret_key_from_str(edr_signer::DangerousSecretKeyStr(AUTHORITY_KEY)).expect("valid key")
}

fn authority() -> Address {
    public_key_to_address(authority_key().public_key())
}

pub fn state(base: &PersistentStateTrie) -> anyhow::Result<PersistentStateTrie> {
    let mut state = base.clone();
    let code = RevmBytecode::new_raw(block_env_contract());
    state.insert_account(
        BLOCK_ENV,
        EdrAccountInfo {
            code_hash: code.hash_slow(),
            code: Some(code),
            ..EdrAccountInfo::default()
        },
    )?;
    let code = RevmBytecode::new_raw(hex!("00").into());
    state.insert_account(
        CODE_CALLER,
        EdrAccountInfo {
            balance: U256::from(10).pow(U256::from(20)),
            code_hash: code.hash_slow(),
            code: Some(code),
            ..EdrAccountInfo::default()
        },
    )?;
    state.insert_account(
        NONCE_3,
        EdrAccountInfo {
            balance: U256::from(10).pow(U256::from(20)),
            nonce: 3,
            ..EdrAccountInfo::default()
        },
    )?;
    state.insert_account(
        authority(),
        EdrAccountInfo {
            balance: U256::from(10).pow(U256::from(18)),
            ..EdrAccountInfo::default()
        },
    )?;
    Ok(state)
}

/// A block header that is valid for `hardfork`.
fn header(hardfork: L1Hardfork, base_fee: u128, excess_blob_gas: u64) -> BlockHeader {
    BlockHeader {
        beneficiary: address!("0x00000000000000000000000000000000000000be"),
        difficulty: if hardfork < L1Hardfork::Merge {
            U256::from(131_072)
        } else {
            U256::ZERO
        },
        number: 100,
        gas_limit: 30_000_000,
        timestamp: 1_700_000_000,
        mix_hash: B256::repeat_byte(0x42),
        base_fee_per_gas: (hardfork >= L1Hardfork::London).then_some(base_fee),
        blob_gas: (hardfork >= L1Hardfork::Cancun).then_some(BlobGas {
            gas_used: 0,
            excess_gas: excess_blob_gas,
        }),
        ..BlockHeader::default()
    }
}

struct RoundTrip {
    name: String,
    cfg: CfgEnv<L1Hardfork>,
    header: BlockHeader,
    transaction: L1SignedTransaction,
    custom_precompiles: HashMap<Address, PrecompileFn>,
}

fn cfg(hardfork: L1Hardfork) -> CfgEnv<L1Hardfork> {
    let mut cfg = CfgEnv::new_with_spec(hardfork);
    cfg.chain_id = CHAIN_ID;
    cfg
}

fn round_trip(name: &str, hardfork: L1Hardfork, transaction: L1SignedTransaction) -> RoundTrip {
    RoundTrip {
        name: name.to_owned(),
        cfg: cfg(hardfork),
        header: header(hardfork, 0, 0),
        transaction,
        custom_precompiles: HashMap::default(),
    }
}

fn legacy(to: TxKind, input: impl Into<Bytes>, gas_limit: u64, nonce: u64) -> Legacy {
    Legacy {
        nonce,
        gas_price: 0,
        gas_limit,
        kind: to,
        value: U256::ZERO,
        input: input.into(),
    }
}

fn eip1559(to: TxKind, input: impl Into<Bytes>) -> Eip1559 {
    Eip1559 {
        chain_id: CHAIN_ID,
        nonce: 0,
        max_priority_fee_per_gas: 2 * GWEI,
        max_fee_per_gas: 10 * GWEI,
        gas_limit: 1_000_000,
        kind: to,
        value: U256::from(7),
        input: input.into(),
        access_list: Vec::new(),
    }
}

fn eip4844(blob_hashes: Vec<B256>) -> Eip4844 {
    Eip4844 {
        chain_id: CHAIN_ID,
        nonce: 0,
        max_priority_fee_per_gas: 2 * GWEI,
        max_fee_per_gas: 10 * GWEI,
        gas_limit: 1_000_000,
        to: BLOCK_ENV,
        value: U256::ZERO,
        input: Bytes::new(),
        access_list: Vec::new(),
        max_fee_per_blob_gas: 100 * GWEI,
        blob_hashes,
    }
}

fn blob_hash(version: u8, index: u8) -> B256 {
    let mut hash = B256::repeat_byte(index);
    hash.0[0] = version;
    hash
}

fn eip7702(authorization_list: Vec<SignedAuthorization>) -> Eip7702 {
    Eip7702 {
        chain_id: CHAIN_ID,
        nonce: 0,
        max_priority_fee_per_gas: GWEI,
        max_fee_per_gas: 10 * GWEI,
        gas_limit: 1_000_000,
        to: authority(),
        value: U256::ZERO,
        input: Bytes::new(),
        access_list: Vec::new(),
        authorization_list,
    }
}

trait IntoRequest {
    fn into_request(self) -> L1TransactionRequest;
}

macro_rules! into_request {
    ($($variant:ident),*) => {
        $(impl IntoRequest for $variant {
            fn into_request(self) -> L1TransactionRequest {
                L1TransactionRequest::$variant(self)
            }
        })*
    };
}

into_request!(Legacy, Eip155, Eip2930, Eip1559, Eip4844, Eip7702);

fn sign(request: impl IntoRequest, caller: Address) -> L1SignedTransaction {
    request.into_request().fake_sign(caller)
}

fn authorization(chain_id: u64, address: Address, nonce: u64) -> SignedAuthorization {
    let authorization = Authorization {
        chain_id: U256::from(chain_id),
        address,
        nonce,
    };
    let signature =
        SignatureWithYParity::with_message(authorization.signature_hash(), &authority_key())
            .expect("signs");
    authorization.into_signed(signature.into_inner())
}

fn p256_input() -> Bytes {
    [
        hex!("4cee90eb86eaa050036147a12d49004b6b9c72bd725d39d4785011fe190f0b4d").as_slice(),
        &hex!("a73bd4903f0ce3b639bbbf6e8e80d16931ff4bcf5993d58468e8fb19086e8cac"),
        &hex!("36dbcd03009df8c59286b162af3bd7fcc0450c9aa81be5d10d312af6c66b1d60"),
        &hex!("4aebd3099c618202fcfe16ae7770b0c49ab5eadf74b754204a3bb6060e44eff3"),
        &hex!("7618b065f9832de4ca6ca971a7a1adc826d0f7c00181a5fb2ddf79ae00b4e10e"),
    ]
    .concat()
    .into()
}

/// EDR's P256VERIFY override, as `edr_napi` builds it.
fn p256_verify(input: &[u8], gas_limit: u64, reservoir: u64) -> PrecompileResult {
    edr_precompile::secp256r1::P256VERIFY.execute(input, gas_limit, reservoir)
}

fn reverting_precompile(input: &[u8], _gas_limit: u64, reservoir: u64) -> PrecompileResult {
    Ok(revm_precompile::PrecompileOutput::revert(
        100,
        Bytes::copy_from_slice(input),
        reservoir,
    ))
}

fn halting_precompile(_input: &[u8], _gas_limit: u64, reservoir: u64) -> PrecompileResult {
    Ok(revm_precompile::PrecompileOutput::halt(
        revm_precompile::PrecompileHalt::OutOfGas,
        reservoir,
    ))
}

/// Calls `target` with `input` from a contract, so a precompile failure shows
/// up inside a frame, then returns the call's success flag and return data.
fn caller_of(target: Address, input: &[u8]) -> Bytes {
    let mut asm = Asm::default();
    for (index, chunk) in input.chunks(32).enumerate() {
        let mut word = [0u8; 32];
        word[..chunk.len()].copy_from_slice(chunk);
        asm = asm.push(&word).push_u(32 * index as u64).op(0x52);
    }
    asm.push_u(0) // out size
        .push_u(0) // out offset
        .push_u(input.len() as u64)
        .push_u(0)
        .push_u(0)
        .push_address(target)
        .push_u(100_000)
        .op(0xf1) // CALL
        .push_u(0)
        .op(0x52) // MSTORE success at 0
        .op(0x3d) // RETURNDATASIZE
        .push_u(0)
        .push_u(32)
        .op(0x3e) // RETURNDATACOPY(32, 0, size)
        .op(0x3d)
        .push_u(32)
        .op(0x01) // ADD
        .push_u(0)
        .op(0xf3) // RETURN(0, 32 + size)
        .build()
}

fn cases(base_cases: &[crate::Case]) -> Vec<RoundTrip> {
    let mut cases = Vec::new();

    // The 48 parity cases, now through EDR's block env and the adapter.
    for case in base_cases {
        let mut round_trip = round_trip(&case.name, case.fork.revm(), crate::revm_transaction(case));
        round_trip.cfg.chain_id = 1;
        cases.push(round_trip);
    }

    let prague = L1Hardfork::Prague;
    let block_env = TxKind::Call(BLOCK_ENV);

    // Block env.
    let mut case = round_trip(
        "block env: all fields",
        prague,
        sign(
            Legacy {
                gas_price: 3 * GWEI,
                ..legacy(block_env, Bytes::new(), 1_000_000, 0)
            },
            CALLER,
        ),
    );
    case.header = header(prague, GWEI, 10_000_000);
    cases.push(case);

    // Transaction types.
    let mut typed = vec![
        round_trip(
            "tx: pre-EIP-155 legacy",
            prague,
            sign(
                Legacy {
                    gas_price: 2 * GWEI,
                    value: U256::from(9),
                    ..legacy(TxKind::Call(RECIPIENT), Bytes::new(), 21_000, 0)
                },
                CALLER,
            ),
        ),
        round_trip(
            "tx: EIP-155 legacy",
            prague,
            sign(
                Eip155 {
                    nonce: 0,
                    gas_price: 2 * GWEI,
                    gas_limit: 1_000_000,
                    kind: block_env,
                    value: U256::ZERO,
                    input: Bytes::new(),
                    chain_id: CHAIN_ID,
                },
                CALLER,
            ),
        ),
        round_trip(
            "tx: EIP-2930 with access list",
            prague,
            sign(
                Eip2930 {
                    chain_id: CHAIN_ID,
                    nonce: 0,
                    gas_price: 2 * GWEI,
                    gas_limit: 1_000_000,
                    kind: TxKind::Call(STORAGE),
                    value: U256::ZERO,
                    input: Bytes::new(),
                    access_list: vec![AccessListItem {
                        address: STORAGE,
                        storage_keys: vec![B256::ZERO, B256::with_last_byte(1)],
                    }],
                },
                CALLER,
            ),
        ),
        round_trip(
            "tx: EIP-1559 with tip",
            prague,
            sign(eip1559(block_env, Bytes::new()), CALLER),
        ),
        round_trip(
            "tx: EIP-1559 create",
            prague,
            sign(
                eip1559(TxKind::Create, hex!("602a60005360016000f3").to_vec()),
                CALLER,
            ),
        ),
        round_trip(
            "tx: EIP-4844 with two blobs",
            prague,
            sign(eip4844(vec![blob_hash(1, 1), blob_hash(1, 2)]), CALLER),
        ),
        round_trip(
            "tx: EIP-7702 delegation",
            prague,
            sign(eip7702(vec![authorization(CHAIN_ID, LOGGER, 0)]), CALLER),
        ),
        round_trip(
            "tx: EIP-7702 any-chain + bad nonce",
            prague,
            sign(
                eip7702(vec![
                    authorization(0, LOGGER, 0),
                    authorization(CHAIN_ID, STORAGE, 7),
                ]),
                CALLER,
            ),
        ),
    ];
    for case in &mut typed {
        case.header = header(prague, GWEI, 10_000_000);
    }
    cases.extend(typed);

    // Config.
    let mut case = round_trip(
        "cfg: wrong chain id",
        prague,
        sign(
            Eip155 {
                nonce: 0,
                gas_price: 0,
                gas_limit: 100_000,
                kind: TxKind::Call(RECIPIENT),
                value: U256::ZERO,
                input: Bytes::new(),
                chain_id: 1,
            },
            CALLER,
        ),
    );
    cases.push(case);

    let call_from_code = sign(legacy(TxKind::Call(RECIPIENT), Bytes::new(), 100_000, 0), CODE_CALLER);
    cases.push(round_trip("cfg: EIP-3607 on", prague, call_from_code.clone()));
    case = round_trip("cfg: EIP-3607 off", prague, call_from_code);
    case.cfg.disable_eip3607 = true;
    cases.push(case);

    let poor = sign(
        Legacy {
            gas_price: GWEI,
            value: U256::from(1_000),
            ..legacy(TxKind::Call(RECIPIENT), Bytes::new(), 100_000, 5)
        },
        POOR,
    );
    case = round_trip("cfg: poor caller, checks on", prague, poor.clone());
    case.header = header(prague, GWEI, 0);
    cases.push(case);
    case = round_trip("cfg: poor caller, set_guarantees", prague, poor);
    case.header = header(prague, GWEI, 0);
    edr_evm::set_guarantees(&mut case.cfg);
    cases.push(case);

    for (name, size) in [("cfg: code size limit, under", 10), ("cfg: code size limit, over", 11)] {
        case = round_trip(
            name,
            prague,
            sign(legacy(TxKind::Create, init_deploying(size), 1_000_000, 0), CALLER),
        );
        case.cfg.limit_contract_code_size = Some(10);
        cases.push(case);
    }
    case = round_trip(
        "cfg: initcode limit (2x code limit)",
        prague,
        sign(legacy(TxKind::Create, vec![0u8; 21], 1_000_000, 0), CALLER),
    );
    case.cfg.limit_contract_code_size = Some(10);
    cases.push(case);

    case = round_trip(
        "cfg: tx gas cap",
        prague,
        sign(legacy(TxKind::Call(RECIPIENT), Bytes::new(), 200_000, 0), CALLER),
    );
    case.cfg.tx_gas_limit_cap = Some(100_000);
    cases.push(case);
    cases.push(round_trip(
        "cfg: Osaka default tx gas cap",
        L1Hardfork::Osaka,
        sign(legacy(TxKind::Call(RECIPIENT), Bytes::new(), 20_000_000, 0), CALLER),
    ));

    let over_block = sign(legacy(TxKind::Call(RECIPIENT), Bytes::new(), 40_000_000, 0), CALLER);
    cases.push(round_trip("cfg: over block gas limit", prague, over_block.clone()));
    case = round_trip("cfg: block gas limit off", prague, over_block);
    case.cfg.disable_block_gas_limit = true;
    cases.push(case);

    // Validation errors.
    let with_header = |mut case: RoundTrip| {
        case.header = header(prague, GWEI, 10_000_000);
        case
    };
    cases.push(round_trip(
        "err: nonce too low",
        prague,
        sign(legacy(TxKind::Call(RECIPIENT), Bytes::new(), 100_000, 1), NONCE_3),
    ));
    cases.push(round_trip(
        "err: nonce too high",
        prague,
        sign(legacy(TxKind::Call(RECIPIENT), Bytes::new(), 100_000, 5), NONCE_3),
    ));
    cases.push(round_trip(
        "err: intrinsic gas",
        prague,
        sign(legacy(TxKind::Call(RECIPIENT), Bytes::new(), 20_000, 0), CALLER),
    ));
    cases.push(round_trip(
        "err: calldata floor (EIP-7623)",
        prague,
        sign(legacy(TxKind::Call(RECIPIENT), vec![1u8; 1_000], 40_000, 0), CALLER),
    ));
    cases.push(with_header(round_trip(
        "err: fee cap below base fee",
        prague,
        sign(
            Eip1559 {
                max_fee_per_gas: GWEI / 2,
                max_priority_fee_per_gas: 0,
                ..eip1559(TxKind::Call(RECIPIENT), Bytes::new())
            },
            CALLER,
        ),
    )));
    cases.push(with_header(round_trip(
        "err: priority fee above max fee",
        prague,
        sign(
            Eip1559 {
                max_priority_fee_per_gas: 20 * GWEI,
                ..eip1559(TxKind::Call(RECIPIENT), Bytes::new())
            },
            CALLER,
        ),
    )));
    cases.push(with_header(round_trip(
        "err: insufficient funds",
        prague,
        sign(
            Eip1559 {
                value: U256::from(10).pow(U256::from(30)),
                ..eip1559(TxKind::Call(RECIPIENT), Bytes::new())
            },
            CALLER,
        ),
    )));
    cases.push(with_header(round_trip(
        "err: empty blobs",
        prague,
        sign(eip4844(Vec::new()), CALLER),
    )));
    cases.push(with_header(round_trip(
        "err: too many blobs",
        prague,
        sign(eip4844((0..10).map(|index| blob_hash(1, index)).collect()), CALLER),
    )));
    cases.push(with_header(round_trip(
        "err: blob version",
        prague,
        sign(eip4844(vec![blob_hash(2, 1)]), CALLER),
    )));
    cases.push(with_header(round_trip(
        "err: blob fee cap below blob base fee",
        prague,
        sign(
            Eip4844 {
                max_fee_per_blob_gas: 1,
                ..eip4844(vec![blob_hash(1, 1)])
            },
            CALLER,
        ),
    )));
    cases.push(with_header(round_trip(
        "err: empty authorization list",
        prague,
        sign(eip7702(Vec::new()), CALLER),
    )));
    for (name, hardfork, transaction) in [
        (
            "err: EIP-2930 on Istanbul",
            L1Hardfork::Istanbul,
            sign(
                Eip2930 {
                    chain_id: CHAIN_ID,
                    nonce: 0,
                    gas_price: 0,
                    gas_limit: 100_000,
                    kind: TxKind::Call(RECIPIENT),
                    value: U256::ZERO,
                    input: Bytes::new(),
                    access_list: Vec::new(),
                },
                CALLER,
            ),
        ),
        (
            "err: EIP-1559 on Berlin",
            L1Hardfork::Berlin,
            sign(eip1559(TxKind::Call(RECIPIENT), Bytes::new()), CALLER),
        ),
        (
            "err: EIP-4844 on Shanghai",
            L1Hardfork::Shanghai,
            sign(eip4844(vec![blob_hash(1, 1)]), CALLER),
        ),
        (
            "err: EIP-7702 on Cancun",
            L1Hardfork::Cancun,
            sign(eip7702(vec![authorization(CHAIN_ID, LOGGER, 0)]), CALLER),
        ),
    ] {
        let mut case = round_trip(name, hardfork, transaction);
        case.header = header(hardfork, GWEI, 0);
        cases.push(case);
    }

    // Custom precompiles, with revm's `PrecompileFn` signature.
    let custom: HashMap<Address, PrecompileFn> = [
        (CUSTOM_PRECOMPILE, p256_verify as PrecompileFn),
        (REVERTING_PRECOMPILE, reverting_precompile as PrecompileFn),
        (HALTING_PRECOMPILE, halting_precompile as PrecompileFn),
        (IDENTITY, p256_verify as PrecompileFn),
    ]
    .into_iter()
    .collect();
    for (name, to, input) in [
        ("precompile: custom P256 (tx)", CUSTOM_PRECOMPILE, p256_input()),
        ("precompile: custom P256, bad input", CUSTOM_PRECOMPILE, Bytes::from(vec![1u8; 160])),
        ("precompile: custom reverts (tx)", REVERTING_PRECOMPILE, Bytes::from(vec![7u8; 4])),
        ("precompile: custom halts (tx)", HALTING_PRECOMPILE, Bytes::new()),
        ("precompile: overrides identity (tx)", IDENTITY, p256_input()),
    ] {
        let mut case = round_trip(
            name,
            prague,
            sign(legacy(TxKind::Call(to), input, 1_000_000, 0), CALLER),
        );
        case.custom_precompiles = custom.clone();
        cases.push(case);
    }
    for (name, target, input) in [
        ("precompile: custom P256 (from contract)", CUSTOM_PRECOMPILE, p256_input()),
        ("precompile: custom reverts (from contract)", REVERTING_PRECOMPILE, Bytes::from(vec![7u8; 4])),
        ("precompile: custom halts (from contract)", HALTING_PRECOMPILE, Bytes::new()),
    ] {
        let mut case = round_trip(
            name,
            prague,
            sign(legacy(TxKind::Create, caller_of(target, &input), 1_000_000, 0), CALLER),
        );
        case.custom_precompiles = custom.clone();
        cases.push(case);
    }

    // Every EDR L1 hardfork, from Byzantium.
    let hardforks = [
        L1Hardfork::Byzantium,
        L1Hardfork::Constantinople,
        L1Hardfork::Petersburg,
        L1Hardfork::Istanbul,
        L1Hardfork::MuirGlacier,
        L1Hardfork::Berlin,
        L1Hardfork::London,
        L1Hardfork::ArrowGlacier,
        L1Hardfork::GrayGlacier,
        L1Hardfork::Merge,
        L1Hardfork::Shanghai,
        L1Hardfork::Cancun,
        L1Hardfork::Prague,
        L1Hardfork::Osaka,
    ];
    for hardfork in hardforks {
        for case in base_cases.iter().filter(|case| {
            [
                "nested calls + creates",
                "storage: clear, restore, write",
                "touch empty/missing accounts, RIPEMD OOG",
                "loop contract",
                "create tx (selfdestruct)",
            ]
            .contains(&case.name.as_str())
        }) {
            let mut round_trip = round_trip(
                &format!("{hardfork:?}: {}", case.name),
                hardfork,
                crate::revm_transaction(case),
            );
            round_trip.header = header(hardfork, 0, 0);
            cases.push(round_trip);
        }
        let mut case = round_trip(
            &format!("{hardfork:?}: block env"),
            hardfork,
            sign(legacy(block_env, Bytes::new(), 1_000_000, 0), CALLER),
        );
        case.header = header(hardfork, 0, 0);
        cases.push(case);
    }

    cases
}

fn revm_trace(state: &PersistentStateTrie, case: &RoundTrip) -> anyhow::Result<CallTraceArena> {
    let mut inspector = TracingInspector::new(
        TracingInspectorConfig::none()
            .set_steps(false)
            .set_record_inputs(true)
            .set_exclude_precompile_calls(false)
            .set_record_logs(true),
    );
    edr_evm::dry_run_with_inspector::<L1ChainSpec, _, _, _, _>(
        FakeBlockHashes,
        state,
        case.cfg.clone(),
        case.transaction.clone(),
        block(case),
        &case.custom_precompiles,
        &mut inspector,
    )
    .map_err(|error| anyhow::anyhow!("revm: {error}"))?;
    Ok(inspector.into_traces())
}

fn block(case: &RoundTrip) -> HeaderAndEvmSpec<'_, BlockHeader, L1Hardfork> {
    HeaderAndEvmSpec {
        hardfork: case.cfg.spec,
        header: &case.header,
        scheduled_blob_params: None,
    }
}

/// Field names that differ between two trace nodes.
fn node_differences(
    revm: &revm_inspectors::tracing::types::CallTraceNode,
    evm2: &revm_inspectors::tracing::types::CallTraceNode,
) -> Vec<String> {
    let (a, b) = (&revm.trace, &evm2.trace);
    let mut fields = Vec::new();
    let mut check = |name: &str, equal: bool, detail: String| {
        if !equal {
            fields.push(format!("{name} ({detail})"));
        }
    };
    check("parent/children", revm.parent == evm2.parent && revm.children == evm2.children, String::new());
    check("depth", a.depth == b.depth, format!("{} vs {}", a.depth, b.depth));
    check("success", a.success == b.success, format!("{} vs {}", a.success, b.success));
    check("caller", a.caller == b.caller, String::new());
    check("address", a.address == b.address, String::new());
    check("maybe_precompile", a.maybe_precompile == b.maybe_precompile, format!("{:?} vs {:?}", a.maybe_precompile, b.maybe_precompile));
    check(
        "selfdestruct",
        (a.selfdestruct_address, a.selfdestruct_refund_target, a.selfdestruct_transferred_value)
            == (b.selfdestruct_address, b.selfdestruct_refund_target, b.selfdestruct_transferred_value),
        format!("{:?} vs {:?}", a.selfdestruct_address, b.selfdestruct_address),
    );
    check("kind", a.kind == b.kind, format!("{:?} vs {:?}", a.kind, b.kind));
    check("value", a.value == b.value, String::new());
    check("data", a.data == b.data, String::new());
    check("output", a.output == b.output, format!("{} vs {}", a.output, b.output));
    check("bytecode", a.bytecode == b.bytecode, String::new());
    check(
        "gas_used",
        a.gas_used == b.gas_used,
        format!("{} vs {}, success {}, status {:?}", a.gas_used, b.gas_used, a.success, a.status),
    );
    check("gas_limit", a.gas_limit == b.gas_limit, format!("{} vs {}", a.gas_limit, b.gas_limit));
    check("gas_refund_counter", a.gas_refund_counter == b.gas_refund_counter, String::new());
    check("status", a.status == b.status, format!("{:?} vs {:?}", a.status, b.status));
    check("logs", revm.logs == evm2.logs, String::new());
    check("ordering", revm.ordering == evm2.ordering, String::new());
    check("idx", revm.idx == evm2.idx, String::new());
    fields
}

pub fn run(base: &PersistentStateTrie, base_cases: &[crate::Case]) -> anyhow::Result<()> {
    let state = state(base)?;
    let cases = cases(base_cases);

    println!("\n== Adapter round trip (EDR inputs -> evm2 -> EDR outputs, vs revm)");
    println!("case | revm | result | precompiles | state root | call trace");
    let (mut result_diffs, mut trace_diffs) = (0, 0);
    for case in &cases {
        let revm = edr_evm::dry_run::<L1ChainSpec, _, _, _>(
            FakeBlockHashes,
            &state,
            case.cfg.clone(),
            case.transaction.clone(),
            block(case),
            &case.custom_precompiles,
        );
        let evm2 = adapter::dry_run(
            FakeBlockHashes,
            &state,
            case.cfg.clone(),
            case.transaction.clone(),
            block(case),
            &case.custom_precompiles,
        );

        match (revm, evm2) {
            (Ok(revm), Ok(evm2)) => {
                let result_equal = revm.result == evm2.result;
                let precompiles_equal = revm.precompile_addresses == evm2.precompile_addresses;
                let (mut revm_state, mut evm2_state) = (state.clone(), state.clone());
                revm_state.commit(revm.state);
                evm2_state.commit(evm2.state);
                let root_equal = revm_state.state_root()? == evm2_state.state_root()?;

                let revm_trace = revm_trace(&state, case)?;
                let evm2_trace = adapter::call_trace(
                    FakeBlockHashes,
                    &state,
                    case.cfg.clone(),
                    case.transaction.clone(),
                    block(case),
                    &case.custom_precompiles,
                )?;
                let mut trace_notes = Vec::new();
                if revm_trace.nodes().len() == evm2_trace.nodes().len() {
                    for (index, (revm, evm2)) in
                        revm_trace.nodes().iter().zip(evm2_trace.nodes()).enumerate()
                    {
                        let fields = node_differences(revm, evm2);
                        if !fields.is_empty() {
                            trace_notes.push(format!("node {index}: {}", fields.join(", ")));
                        }
                    }
                } else {
                    trace_notes.push(format!(
                        "{} vs {} nodes",
                        revm_trace.nodes().len(),
                        evm2_trace.nodes().len()
                    ));
                }

                println!(
                    "{} | {} | {} | {} | {} | {}",
                    case.name,
                    outcome_name(&revm.result),
                    crate::yes_no(result_equal),
                    crate::yes_no(precompiles_equal),
                    crate::yes_no(root_equal),
                    crate::yes_no(trace_notes.is_empty()),
                );
                if !(result_equal && precompiles_equal && root_equal) {
                    result_diffs += 1;
                }
                if !result_equal {
                    println!("  revm: {:?}\n  evm2: {:?}", revm.result, evm2.result);
                }
                if !trace_notes.is_empty() {
                    trace_diffs += 1;
                    for note in trace_notes {
                        println!("  trace {note}");
                    }
                }
            }
            (Err(revm), Err(evm2)) => {
                let (revm, evm2) = (format!("{revm:?}"), format!("{evm2:?}"));
                println!(
                    "{} | error | {} | - | - | -",
                    case.name,
                    crate::yes_no(revm == evm2)
                );
                if revm != evm2 {
                    result_diffs += 1;
                    println!("  revm: {revm}\n  evm2: {evm2}");
                } else {
                    println!("  {revm}");
                }
            }
            (revm, evm2) => {
                result_diffs += 1;
                println!("{} | MISMATCH", case.name);
                println!(
                    "  revm: {:?}\n  evm2: {:?}",
                    revm.map(|ok| ok.result),
                    evm2.map(|ok| ok.result)
                );
            }
        }
    }
    println!(
        "\n{result_diffs} result difference(s), {trace_diffs} call-trace difference(s) in {} cases",
        cases.len()
    );
    Ok(())
}

fn outcome_name(result: &edr_chain_spec_evm::result::ExecutionResult<edr_chain_l1::HaltReason>) -> String {
    use edr_chain_spec_evm::result::ExecutionResult;
    match result {
        ExecutionResult::Success { reason, .. } => format!("{reason:?}"),
        ExecutionResult::Revert { .. } => "Revert".to_owned(),
        ExecutionResult::Halt { reason, .. } => format!("{reason:?}"),
    }
}
