//! Spike: runs the same L1 transactions on revm (through EDR's chain spec) and
//! on evm2, both over EDR's own state, and compares:
//! - the transaction outcome (status, output, gas, logs, state changes),
//! - opcode steps recorded the way EDR's Solidity tracer does,
//! - the call-level `CallTraceArena` of revm-inspectors and evm2-inspectors.
//!
//! Run with `cargo run --release -p edr_spike_evm2`.

use std::{
    cell::Cell,
    time::{Duration, Instant},
};

use alloy_consensus::{transaction::Recovered, TxLegacy};
use alloy_primitives::{hex, TxKind};
use edr_blockchain_api::BlockHashByNumber;
use edr_chain_l1::{request::Legacy, L1ChainSpec, L1Hardfork, L1TransactionRequest};
use edr_chain_spec_evm::{
    interpreter::{EthInterpreter, Interpreter},
    result::{ExecutionResult, Output},
    CfgEnv, ContextTrait, Inspector,
};
use edr_primitives::{address, Address, Bytecode as RevmBytecode, Bytes, HashMap, B256, U256};
use edr_signer::FakeSign as _;
use edr_state_api::{
    account::{Account, AccountInfo as EdrAccountInfo},
    EvmState, EvmStorageSlot, State, StateCommit as _, StateDebug as _, StateError, TransactionId,
};
use edr_state_persistent_trie::PersistentStateTrie;
use evm2::{
    bytecode::Bytecode as Evm2Bytecode,
    env::BlockEnvExt,
    ethereum::{ethereum_tx_registry, TxEnvelope},
    evm::{Database, Db},
    BaseEvmTypes, Evm, Precompiles,
};
use revm_interpreter::interpreter_types::Jumps as _;

const CALLER: Address = address!("0x00000000000000000000000000000000000000aa");
const RECIPIENT: Address = address!("0x00000000000000000000000000000000000000bb");
const LOOP: Address = address!("0x00000000000000000000000000000000000000cc");
const REVERTER: Address = address!("0x00000000000000000000000000000000000000d1");
const INVALID_FE: Address = address!("0x00000000000000000000000000000000000000d2");
const INFINITE_LOOP: Address = address!("0x00000000000000000000000000000000000000d3");
const STACK_UNDERFLOW: Address = address!("0x00000000000000000000000000000000000000d4");
const INVALID_JUMP: Address = address!("0x00000000000000000000000000000000000000d5");
const NESTED: Address = address!("0x00000000000000000000000000000000000000d6");
const STATIC_WRITER: Address = address!("0x00000000000000000000000000000000000000d7");
const LOGGER: Address = address!("0x00000000000000000000000000000000000000d8");
const UNDEFINED_OPCODE: Address = address!("0x00000000000000000000000000000000000000d9");
const RECURSIVE: Address = address!("0x00000000000000000000000000000000000000da");
const FRESH: Address = address!("0x00000000000000000000000000000000000000ee");
const STORAGE: Address = address!("0x00000000000000000000000000000000000000db");
const SELFDESTRUCT_EXISTING: Address = address!("0x00000000000000000000000000000000000000dc");
const SELFDESTRUCT_TO_SELF: Address = address!("0x00000000000000000000000000000000000000dd");
const TOUCHER: Address = address!("0x00000000000000000000000000000000000000de");
/// An empty account (zero balance and nonce, no code) stored in the trie.
const EMPTY: Address = address!("0x00000000000000000000000000000000000000e0");
const EMPTY_DIRECT: Address = address!("0x00000000000000000000000000000000000000e1");
const MISSING: Address = address!("0x00000000000000000000000000000000000000e2");

const LOOP_ITERATIONS: u16 = 10_000;
const SUB_CALL_GAS: u64 = 100_000;
const GAS_LIMIT: u64 = 10_000_000;
const WARMUP_RUNS: usize = 3;
const MEASURED_RUNS: usize = 20;

/// Loops `iterations` times over arithmetic, `MSTORE` and `SLOAD`, then writes
/// a storage slot and emits a log.
fn loop_contract(iterations: u16) -> Bytes {
    let [hi, lo] = iterations.to_be_bytes();
    vec![
        0x61, hi, lo,   // PUSH2 iterations
        0x5b, // 3: JUMPDEST
        0x80, 0x80, 0x02, // DUP1 DUP1 MUL
        0x60, 0x00, 0x52, // PUSH1 0 MSTORE
        0x80, 0x60, 0x0f, 0x16, // DUP1 PUSH1 15 AND
        0x54, 0x50, // SLOAD POP
        0x60, 0x01, 0x90, 0x03, // PUSH1 1 SWAP1 SUB
        0x80, 0x60, 0x03, 0x57, // DUP1 PUSH1 3 JUMPI
        0x60, 0x2a, 0x60, 0x01, 0x55, // PUSH1 42 PUSH1 1 SSTORE
        0x60, 0x20, 0x60, 0x00, 0xa0, // PUSH1 32 PUSH1 0 LOG0
        0x00, // STOP
    ]
    .into()
}

/// Straight-line bytecode builder.
#[derive(Default)]
struct Asm(Vec<u8>);

impl Asm {
    fn op(mut self, opcode: u8) -> Self {
        self.0.push(opcode);
        self
    }

    /// `PUSHn` with the minimal width (at least one byte).
    fn push(mut self, value: &[u8]) -> Self {
        let first = value.iter().position(|byte| *byte != 0);
        let value = first.map_or(&[0u8][..], |first| &value[first..]);
        assert!(value.len() <= 32);
        self.0.push(0x5f + u8::try_from(value.len()).expect("fits"));
        self.0.extend_from_slice(value);
        self
    }

    fn push_u(self, value: u64) -> Self {
        self.push(&value.to_be_bytes())
    }

    fn push_address(self, address: Address) -> Self {
        // Keeps all 20 bytes so the push width is stable.
        let mut asm = self;
        asm.0.push(0x73);
        asm.0.extend_from_slice(address.as_slice());
        asm
    }

    /// `CALL`-family call without arguments, storing the success flag in
    /// `slot`.
    /// Forwards `SUB_CALL_GAS`, or all gas (`GAS`) for `RECURSIVE`.
    fn call(self, opcode: u8, target: Address, value: Option<u64>, slot: u64) -> Self {
        let asm = self.push_u(0).push_u(0).push_u(0).push_u(0);
        let asm = match value {
            Some(value) => asm.push_u(value),
            None => asm,
        };
        let asm = asm.push_address(target);
        let asm = if target == RECURSIVE {
            asm.op(0x5a) // GAS
        } else {
            asm.push_u(SUB_CALL_GAS)
        };
        asm.op(opcode).push_u(slot).op(0x55) // SSTORE
    }

    /// Writes `init` to memory at offset 0 and runs `CREATE` (or `CREATE2`
    /// with `salt`), storing the created address in `slot`.
    fn create(self, init: &[u8], value: u64, salt: Option<u64>, slot: u64) -> Self {
        let mut asm = self;
        for (index, chunk) in init.chunks(32).enumerate() {
            let mut word = [0u8; 32];
            word[..chunk.len()].copy_from_slice(chunk);
            asm = asm
                .push(&word)
                .push_u(32 * u64::try_from(index).expect("fits"))
                .op(0x52); // MSTORE
        }
        let size = u64::try_from(init.len()).expect("fits");
        let asm = match salt {
            Some(salt) => asm
                .push_u(salt)
                .push_u(size)
                .push_u(0)
                .push_u(value)
                .op(0xf5),
            None => asm.push_u(size).push_u(0).push_u(value).op(0xf0),
        };
        asm.push_u(slot).op(0x55)
    }

    fn build(self) -> Bytes {
        self.0.into()
    }
}

/// Init code deploying a one-byte runtime `0x2a`.
const INIT_RETURNS_CODE: [u8; 10] = hex!("602a60005360016000f3");
/// Init code that reverts with no data.
const INIT_REVERTS: [u8; 5] = hex!("60006000fd");
/// Init code that halts on `INVALID`.
const INIT_INVALID: [u8; 1] = hex!("fe");
/// Init code that deploys runtime starting with `0xEF` (EIP-3541).
const INIT_EF_CODE: [u8; 10] = hex!("60ef60005360016000f3");

/// Init code that self-destructs to `RECIPIENT`.
fn init_selfdestruct() -> Bytes {
    Asm::default().push_address(RECIPIENT).op(0xff).build()
}

fn nested_contract() -> Bytes {
    Asm::default()
        // Sub-call that reverts.
        .call(0xf1, REVERTER, Some(0), 1)
        // Static call that tries to write.
        .call(0xfa, STATIC_WRITER, None, 2)
        // Delegate call that logs and writes in this contract's storage.
        .call(0xf4, LOGGER, None, 3)
        // Precompile call with input: identity(word) into memory 32..64.
        .push(&[0x11; 32])
        .push_u(0)
        .op(0x52)
        .push_u(32) // retSize
        .push_u(32) // retOffset
        .push_u(32) // argsSize
        .push_u(0) // argsOffset
        .push_u(0) // value
        .push_u(4)
        .op(0x5a)
        .op(0xf1)
        .push_u(4)
        .op(0x55)
        .push_u(32)
        .op(0x51)
        .push_u(5)
        .op(0x55)
        // Creations.
        .create(&INIT_RETURNS_CODE, 0, None, 6)
        .create(&INIT_RETURNS_CODE, 0, Some(7), 7)
        .create(&INIT_REVERTS, 0, None, 8)
        .create(&init_selfdestruct(), 5, None, 9)
        // Value transfer to a fresh account.
        .call(0xf1, FRESH, Some(1), 11)
        // Call to an account without code.
        .call(0xf1, RECIPIENT, Some(0), 12)
        // Undefined opcode in a sub-call.
        .call(0xf1, UNDEFINED_OPCODE, Some(0), 13)
        // Own log.
        .push_u(0xbeef)
        .push_u(0xcafe)
        .push_u(0)
        .push_u(0)
        .op(0xa2) // LOG2
        // Last: a failed CREATE burns all the gas it was given (63/64).
        .create(&INIT_EF_CODE, 0, None, 10)
        .op(0x00)
        .build()
}

/// Touches empty and missing accounts without value, and calls the RIPEMD-160
/// precompile with too little gas (the EIP-161 precompile-3 rule).
fn toucher_contract() -> Bytes {
    let mut asm = Asm::default()
        .call(0xf1, EMPTY, Some(0), 1)
        .call(0xf1, MISSING, Some(0), 2)
        .call(0xfa, FRESH, None, 3);
    // CALL(gas = 1, 0x03, value 0, no args, no return)
    asm = asm
        .push_u(0)
        .push_u(0)
        .push_u(0)
        .push_u(0)
        .push_u(0)
        .push_u(3)
        .push_u(1)
        .op(0xf1)
        .push_u(4)
        .op(0x55);
    asm.op(0x00).build()
}

/// Calls itself until the call depth limit.
fn recursive_contract() -> Bytes {
    Asm::default()
        .push_u(1)
        .push_u(0)
        .op(0x54) // SLOAD
        .op(0x01) // ADD
        .push_u(0)
        .op(0x55) // SSTORE
        .call(0xf1, RECURSIVE, Some(0), 1)
        .op(0x00)
        .build()
}

fn edr_state() -> anyhow::Result<PersistentStateTrie> {
    let mut state = PersistentStateTrie::default();
    state.insert_account(
        CALLER,
        EdrAccountInfo {
            balance: U256::from(10).pow(U256::from(24)),
            ..EdrAccountInfo::default()
        },
    )?;

    for address in [EMPTY, EMPTY_DIRECT] {
        state.insert_account(address, EdrAccountInfo::default())?;
    }

    let contracts: [(Address, Bytes); 16] = [
        (LOOP, loop_contract(LOOP_ITERATIONS)),
        (
            REVERTER,
            hex!("600160005563deadbeef6000526004601cfd").into(),
        ),
        (INVALID_FE, hex!("fe").into()),
        (INFINITE_LOOP, hex!("5b600056").into()),
        (STACK_UNDERFLOW, hex!("01").into()),
        (INVALID_JUMP, hex!("600556").into()),
        (NESTED, nested_contract()),
        (STATIC_WRITER, hex!("600160005500").into()),
        (LOGGER, hex!("6007600555602a6000600060a100").into()),
        (UNDEFINED_OPCODE, hex!("0c").into()),
        (RECURSIVE, recursive_contract()),
        (RECIPIENT, Bytes::new()),
        // Clears slot 0, changes slot 1 and restores it, writes slot 2.
        (
            STORAGE,
            hex!("6000600055600860015560076001556009600255").into(),
        ),
        (SELFDESTRUCT_EXISTING, init_selfdestruct()),
        (SELFDESTRUCT_TO_SELF, hex!("30ff").into()),
        (TOUCHER, toucher_contract()),
    ];
    for (address, code) in contracts {
        if code.is_empty() {
            continue;
        }
        let code = RevmBytecode::new_raw(code);
        state.insert_account(
            address,
            EdrAccountInfo {
                balance: match address {
                    NESTED => U256::from(1_000),
                    SELFDESTRUCT_EXISTING | SELFDESTRUCT_TO_SELF => U256::from(50),
                    _ => U256::ZERO,
                },
                code_hash: code.hash_slow(),
                code: Some(code),
                ..EdrAccountInfo::default()
            },
        )?;
    }

    state.set_account_storage_slot(STORAGE, U256::from(0), U256::from(5))?;
    state.set_account_storage_slot(STORAGE, U256::from(1), U256::from(7))?;

    Ok(state)
}

#[derive(Clone, Copy, Debug)]
enum Fork {
    Prague,
    Osaka,
}

impl Fork {
    fn revm(self) -> L1Hardfork {
        match self {
            Fork::Prague => L1Hardfork::Prague,
            Fork::Osaka => L1Hardfork::Osaka,
        }
    }

    fn evm2(self) -> evm2::SpecId {
        match self {
            Fork::Prague => evm2::SpecId::PRAGUE,
            Fork::Osaka => evm2::SpecId::OSAKA,
        }
    }
}

/// A transaction, described once and built for each engine.
#[derive(Clone)]
struct Case {
    name: String,
    fork: Fork,
    kind: TxKind,
    value: u64,
    input: Bytes,
    gas_limit: u64,
    gas_price: u128,
    nonce: u64,
}

fn call(name: &str, to: Address) -> Case {
    Case {
        name: name.to_owned(),
        fork: Fork::Prague,
        kind: TxKind::Call(to),
        value: 0,
        input: Bytes::new(),
        gas_limit: GAS_LIMIT,
        gas_price: 0,
        nonce: 0,
    }
}

fn create(name: &str, init: impl Into<Bytes>) -> Case {
    Case {
        kind: TxKind::Create,
        input: init.into(),
        ..call(name, Address::ZERO)
    }
}

fn precompile(name: &str, address: u64, input: Vec<u8>) -> Case {
    let mut bytes = [0u8; 20];
    bytes[12..].copy_from_slice(&address.to_be_bytes());
    Case {
        input: input.into(),
        ..call(name, Address::from(bytes))
    }
}

fn word(value: u64) -> [u8; 32] {
    U256::from(value).to_be_bytes()
}

fn precompile_cases() -> Vec<Case> {
    let ecrecover = [
        hex!("456e9aea5e197a1f1af7a3e85a3212fa4049a3ba34c2289b4c860fc0b0c64ef3").as_slice(),
        &word(28),
        &hex!("9242685bf161793cc25603c231bc2f568eb630ea16aa137d2664ac8038825608"),
        &hex!("4f8ae3bd7535248d0bd448298cc2e2071e56992d0774dc340c368ae950852ada"),
    ]
    .concat();
    let modexp = [word(1).as_slice(), &word(1), &word(1), &[3, 5, 7]].concat();
    let modexp_large = [word(64).as_slice(), &word(64), &word(64), &[0xff; 192]].concat();
    let g1 = [word(1), word(2)].concat();
    let bn_add = [g1.clone(), g1.clone()].concat();
    let bn_mul = [g1.clone(), word(2).to_vec()].concat();
    let bn_invalid = [word(1), word(3), word(1), word(2)].concat();
    // EIP-152 test vector 5: 12 rounds over "abc".
    let mut abc = [0u8; 128];
    abc[..3].copy_from_slice(b"abc");
    let blake2f = [
        hex!("0000000c").as_slice(),
        &hex!("48c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b"),
        &abc,
        &3u64.to_le_bytes(),
        &0u64.to_le_bytes(),
        &[1],
    ]
    .concat();
    let commitment = hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7");
    let mut versioned_hash: [u8; 32] = sha256(&commitment);
    versioned_hash[0] = 1;
    let point_evaluation = [
        versioned_hash.as_slice(),
        &hex!("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000000"),
        &hex!("1522a4a7f34e1ea350ae07c29c96c7e79655aa926122e95fe69fcbd932ca49e9"),
        &commitment,
        &hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c"),
    ]
    .concat();
    let p256 = [
        hex!("4cee90eb86eaa050036147a12d49004b6b9c72bd725d39d4785011fe190f0b4d").as_slice(),
        &hex!("a73bd4903f0ce3b639bbbf6e8e80d16931ff4bcf5993d58468e8fb19086e8cac"),
        &hex!("36dbcd03009df8c59286b162af3bd7fcc0450c9aa81be5d10d312af6c66b1d60"),
        &hex!("4aebd3099c618202fcfe16ae7770b0c49ab5eadf74b754204a3bb6060e44eff3"),
        &hex!("7618b065f9832de4ca6ca971a7a1adc826d0f7c00181a5fb2ddf79ae00b4e10e"),
    ]
    .concat();

    let mut cases = vec![
        precompile("ecrecover", 1, ecrecover),
        precompile("ecrecover (garbage)", 1, vec![0xab; 128]),
        precompile("sha256", 2, b"edr".to_vec()),
        precompile("ripemd160", 3, b"edr".to_vec()),
        precompile("identity", 4, vec![0x42; 100]),
        precompile("modexp", 5, modexp),
        precompile("modexp (64-byte operands)", 5, modexp_large.clone()),
        precompile("bn254 add", 6, bn_add),
        precompile("bn254 add (invalid point)", 6, bn_invalid),
        precompile("bn254 mul", 7, bn_mul),
        precompile("bn254 pairing (empty)", 8, Vec::new()),
        precompile("blake2f", 9, blake2f),
        precompile("blake2f (bad length)", 9, vec![0; 10]),
        precompile("point evaluation", 0x0a, point_evaluation),
        precompile("bls12 g1 add (infinity)", 0x0b, vec![0; 256]),
        precompile("bls12 g1 msm (infinity)", 0x0c, vec![0; 160]),
        precompile("bls12 g2 add (infinity)", 0x0d, vec![0; 512]),
        precompile("bls12 g2 msm (infinity)", 0x0e, vec![0; 288]),
        precompile("bls12 pairing (infinity)", 0x0f, vec![0; 384]),
        precompile("bls12 map fp to g1", 0x10, vec![0; 64]),
        precompile("bls12 map fp2 to g2", 0x11, vec![0; 128]),
        precompile("bls12 g1 add (bad length)", 0x0b, vec![0; 10]),
        precompile("p256verify (inactive)", 0x100, p256.clone()),
    ];
    for (name, case) in [
        ("modexp (64-byte operands)", precompile("", 5, modexp_large)),
        ("p256verify", precompile("", 0x100, p256)),
    ] {
        cases.push(Case {
            name: name.to_owned(),
            fork: Fork::Osaka,
            ..case
        });
    }
    cases
}

fn sha256(input: &[u8]) -> [u8; 32] {
    use sha2::Digest as _;
    sha2::Sha256::digest(input).into()
}

fn cases() -> Vec<Case> {
    let mut cases = vec![
        Case {
            value: 1,
            ..call("transfer", RECIPIENT)
        },
        call("loop contract", LOOP),
        call("revert with data", REVERTER),
        call("halt: INVALID (0xfe)", INVALID_FE),
        call("halt: undefined opcode", UNDEFINED_OPCODE),
        Case {
            gas_limit: 100_000,
            ..call("halt: out of gas", INFINITE_LOOP)
        },
        call("halt: stack underflow", STACK_UNDERFLOW),
        call("halt: invalid jump", INVALID_JUMP),
        Case {
            value: 100,
            ..call("nested calls + creates", NESTED)
        },
        call("recursion to depth limit", RECURSIVE),
        create("create tx", INIT_RETURNS_CODE),
        create("create tx (init reverts)", INIT_REVERTS),
        create("create tx (init halts)", INIT_INVALID),
        create("create tx (0xEF code)", INIT_EF_CODE),
        create("create tx (selfdestruct)", init_selfdestruct()),
        Case {
            value: 7,
            ..create("create tx (runs loop)", loop_contract(100))
        },
        // State edge cases (EIP-161 touches, selfdestruct, storage).
        call("zero-value tx to an empty account", EMPTY_DIRECT),
        call("zero-value tx to a missing account", MISSING),
        call("touch empty/missing accounts, RIPEMD OOG", TOUCHER),
        call("storage: clear, restore, write", STORAGE),
        call("selfdestruct existing contract", SELFDESTRUCT_EXISTING),
        call("selfdestruct to self", SELFDESTRUCT_TO_SELF),
        Case {
            gas_price: 1,
            ..call("gas price 1 (coinbase paid)", LOOP)
        },
    ];
    cases.extend(precompile_cases());
    cases
}

/// Opcode steps, recorded like EDR's `StepRecorder`.
#[derive(Default)]
struct Steps {
    record_stack: bool,
    pcs: Vec<u32>,
    opcodes: Vec<u8>,
    stacks: Vec<Box<[U256]>>,
}

impl Steps {
    fn new(record_stack: bool) -> Self {
        Self {
            record_stack,
            ..Self::default()
        }
    }

    fn record(&mut self, pc: usize, opcode: u8, stack: impl FnOnce() -> Box<[U256]>) {
        self.pcs.push(u32::try_from(pc).expect("pc fits in u32"));
        self.opcodes.push(opcode);
        if self.record_stack {
            self.stacks.push(stack());
        }
    }
}

struct RevmRecorder(Steps);

impl<ContextT: ContextTrait> Inspector<ContextT, EthInterpreter> for RevmRecorder {
    fn step(&mut self, interp: &mut Interpreter<EthInterpreter>, _context: &mut ContextT) {
        let pc = interp.bytecode.pc();
        let opcode = interp.bytecode.opcode();
        self.0
            .record(pc, opcode, || interp.stack.data().as_slice().into());
    }
}

struct Evm2Recorder(Steps);

impl evm2::Inspector<BaseEvmTypes> for Evm2Recorder {
    fn step(&mut self, interp: &mut evm2::interpreter::Interpreter<'_, '_, BaseEvmTypes>) {
        let pc = interp.pc();
        let opcode = interp.opcode();
        self.0
            .record(pc, opcode, || interp.stack().as_slice().into());
    }
}

/// evm2 database reading EDR's state.
struct EdrStateDatabase<'state> {
    state: &'state PersistentStateTrie,
    code_loads: &'state Cell<u32>,
    code_conversion: &'state Cell<Duration>,
}

impl Database for EdrStateDatabase<'_> {
    type Error = StateError;

    fn get_account(&mut self, address: &Address) -> Result<Option<evm2::AccountInfo>, StateError> {
        Ok(self
            .state
            .basic(*address)?
            .map(|info| evm2::AccountInfo::new(info.balance, info.nonce, info.code_hash, None)))
    }

    fn get_code_by_hash(&mut self, code_hash: &B256) -> Result<Evm2Bytecode, StateError> {
        let code = self.state.code_by_hash(*code_hash)?;

        let start = Instant::now();
        let converted = Evm2Bytecode::new_raw(code.original_bytes());
        self.code_conversion
            .set(self.code_conversion.get() + start.elapsed());
        self.code_loads.set(self.code_loads.get() + 1);

        Ok(converted)
    }

    fn get_storage(&mut self, address: &Address, key: &U256) -> Result<U256, StateError> {
        self.state.storage(*address, *key)
    }

    fn get_block_hash(&mut self, _number: &U256) -> Result<B256, StateError> {
        Ok(B256::ZERO)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("no block hashes in the spike")]
struct NoBlockHashError;

struct NoBlockHashes;

impl BlockHashByNumber for NoBlockHashes {
    type Error = NoBlockHashError;

    fn block_hash_by_number(&self, _block_number: u64) -> Result<B256, Self::Error> {
        Err(NoBlockHashError)
    }
}

/// A changed account: address, nonce, balance, code hash and changed storage
/// slots.
type AccountChange = (Address, u64, U256, B256, Vec<(U256, U256)>);

/// What both engines must agree on.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    /// Normalized stop reason, e.g. `Return`, `Revert`, `InvalidFEOpcode`.
    stop: String,
    output: Bytes,
    created_address: Option<Address>,
    gas_used: u64,
    logs: Vec<(Address, Vec<B256>, Bytes)>,
    changes: Vec<AccountChange>,
}

/// Maps revm's result to evm2's `InstrStop` names.
fn revm_stop(result: &ExecutionResult<edr_chain_l1::HaltReason>) -> String {
    match result {
        ExecutionResult::Success { reason, .. } => format!("{reason:?}"),
        ExecutionResult::Revert { .. } => "Revert".to_owned(),
        ExecutionResult::Halt { reason, .. } => {
            let name = format!("{reason:?}");
            match name.as_str() {
                "OutOfGas(Basic)" => "OutOfGas".to_owned(),
                "OutOfGas(Memory)" => "MemoryOOG".to_owned(),
                "OutOfGas(MemoryLimit)" => "MemoryLimitOOG".to_owned(),
                "OutOfGas(Precompile)" => "PrecompileOOG".to_owned(),
                "OutOfGas(InvalidOperand)" => "InvalidOperandOOG".to_owned(),
                "OutOfGas(ReentrancySentry)" => "ReentrancySentryOOG".to_owned(),
                name if name.starts_with("PrecompileErrorWithContext") => {
                    "PrecompileError".to_owned()
                }
                _ => name,
            }
        }
    }
}

fn revm_transaction(case: &Case) -> edr_chain_l1::L1SignedTransaction {
    L1TransactionRequest::Legacy(Legacy {
        nonce: case.nonce,
        gas_price: case.gas_price,
        gas_limit: case.gas_limit,
        kind: case.kind,
        value: U256::from(case.value),
        input: case.input.clone(),
    })
    .fake_sign(CALLER)
}

fn revm_block() -> revm_context::BlockEnv {
    revm_context::BlockEnv {
        gas_limit: 30_000_000,
        ..revm_context::BlockEnv::default()
    }
}

fn run_revm(
    state: &PersistentStateTrie,
    case: &Case,
    inspector: Option<&mut RevmRecorder>,
) -> anyhow::Result<Outcome> {
    let transaction = revm_transaction(case);
    let cfg = CfgEnv::new_with_spec(case.fork.revm());

    let execution = if let Some(inspector) = inspector {
        edr_evm::dry_run_with_inspector::<L1ChainSpec, _, _, _, _>(
            NoBlockHashes,
            state,
            cfg,
            transaction,
            revm_block(),
            &HashMap::default(),
            inspector,
        )
    } else {
        edr_evm::dry_run::<L1ChainSpec, _, _, _>(
            NoBlockHashes,
            state,
            cfg,
            transaction,
            revm_block(),
            &HashMap::default(),
        )
    }
    .map_err(|error| anyhow::anyhow!("revm: {error}"))?;

    let mut changes: Vec<_> = execution
        .state
        .iter()
        .filter(|(_, account)| account.is_touched())
        .map(|(address, account)| {
            let mut slots: Vec<_> = account
                .storage
                .iter()
                .filter(|(_, slot)| slot.is_changed())
                .map(|(key, slot)| (*key, slot.present_value()))
                .collect();
            slots.sort();
            let (nonce, balance, code_hash) = if account.is_selfdestructed() {
                (0, U256::ZERO, edr_primitives::KECCAK_EMPTY)
            } else {
                (
                    account.info.nonce,
                    account.info.balance,
                    account.info.code_hash,
                )
            };
            (*address, nonce, balance, code_hash, slots)
        })
        .collect();
    changes.sort();

    let (output, created_address) = match &execution.result {
        ExecutionResult::Success {
            output: Output::Call(bytes),
            ..
        } => (bytes.clone(), None),
        ExecutionResult::Success {
            output: Output::Create(bytes, address),
            ..
        } => (bytes.clone(), *address),
        ExecutionResult::Revert { output, .. } => (output.clone(), None),
        ExecutionResult::Halt { .. } => (Bytes::new(), None),
    };

    Ok(Outcome {
        stop: revm_stop(&execution.result),
        output,
        created_address,
        gas_used: execution.result.tx_gas_used(),
        logs: execution
            .result
            .logs()
            .iter()
            .map(|log| {
                (
                    log.address,
                    log.data.topics().to_vec(),
                    log.data.data.clone(),
                )
            })
            .collect(),
        changes,
    })
}

fn revm_traces(
    state: &PersistentStateTrie,
    case: &Case,
) -> anyhow::Result<revm_inspectors::tracing::CallTraceArena> {
    use revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};

    // Same call-level config as EDR's `SolidityTracingInspector`.
    let mut inspector = TracingInspector::new(
        TracingInspectorConfig::none()
            .set_steps(false)
            .set_record_inputs(true)
            .set_exclude_precompile_calls(false)
            .set_record_logs(true),
    );
    edr_evm::dry_run_with_inspector::<L1ChainSpec, _, _, _, _>(
        NoBlockHashes,
        state,
        CfgEnv::new_with_spec(case.fork.revm()),
        revm_transaction(case),
        revm_block(),
        &HashMap::default(),
        &mut inspector,
    )
    .map_err(|error| anyhow::anyhow!("revm: {error}"))?;
    Ok(inspector.into_traces())
}

struct Evm2Run {
    outcome: Outcome,
    code_loads: u32,
    code_conversion: Duration,
}

fn evm2_transaction(case: &Case) -> Recovered<TxEnvelope> {
    Recovered::new_unchecked(
        TxEnvelope::Legacy(TxLegacy {
            to: case.kind,
            value: U256::from(case.value),
            input: case.input.clone(),
            gas_limit: case.gas_limit,
            gas_price: case.gas_price,
            nonce: case.nonce,
            ..TxLegacy::default()
        }),
        CALLER,
    )
}

fn evm2_evm<'state>(
    state: &'state PersistentStateTrie,
    case: &Case,
    code_loads: &'state Cell<u32>,
    code_conversion: &'state Cell<Duration>,
) -> Evm<'state, BaseEvmTypes> {
    let database = EdrStateDatabase {
        state,
        code_loads,
        code_conversion,
    };
    let spec = case.fork.evm2();
    let block = BlockEnvExt {
        gas_limit: U256::from(30_000_000),
        ..BlockEnvExt::default()
    };
    Evm::<BaseEvmTypes>::new(
        spec,
        block,
        ethereum_tx_registry(spec),
        Db::new(database),
        Precompiles::base(spec),
    )
}

fn run_evm2(
    state: &PersistentStateTrie,
    case: &Case,
    inspector: Option<&mut Evm2Recorder>,
) -> anyhow::Result<Evm2Run> {
    let code_loads = Cell::new(0);
    let code_conversion = Cell::new(Duration::ZERO);
    let mut evm = evm2_evm(state, case, &code_loads, &code_conversion);
    let transaction = evm2_transaction(case);

    let executed = match inspector {
        Some(inspector) => evm.transact_with_inspector(&transaction, inspector),
        None => evm.transact(&transaction),
    }
    .map_err(|error| anyhow::anyhow!("evm2: {error:?}"))?;

    let mut changes: Vec<_> = executed
        .account_changes()
        .filter(|change| change.touched || change.is_changed())
        .map(|change| {
            let (nonce, balance, code_hash) = change
                .current
                .map_or((0, U256::ZERO, edr_primitives::KECCAK_EMPTY), |info| {
                    (info.nonce, info.balance, info.code_hash)
                });
            let mut slots: Vec<_> = change
                .storage
                .slots
                .iter()
                .filter(|(_, slot)| slot.value.is_changed())
                .map(|(key, slot)| (*key, slot.value.current))
                .collect();
            slots.sort();
            (change.address, nonce, balance, code_hash, slots)
        })
        .collect();
    changes.sort();

    let result = executed.discard();

    Ok(Evm2Run {
        outcome: Outcome {
            stop: format!("{:?}", result.stop),
            output: result.output.clone(),
            created_address: result.created_address,
            gas_used: result.tx_gas_used(),
            logs: result
                .logs
                .iter()
                .map(|log| {
                    (
                        log.address,
                        log.data.topics().to_vec(),
                        log.data.data.clone(),
                    )
                })
                .collect(),
            changes,
        },
        code_loads: code_loads.get(),
        code_conversion: code_conversion.get(),
    })
}

fn evm2_traces(
    state: &PersistentStateTrie,
    case: &Case,
) -> anyhow::Result<evm2_inspectors::tracing::CallTraceArena> {
    use evm2_inspectors::tracing::{TracingInspector, TracingInspectorConfig};

    let code_loads = Cell::new(0);
    let code_conversion = Cell::new(Duration::ZERO);
    let mut evm = evm2_evm(state, case, &code_loads, &code_conversion);
    let mut inspector = TracingInspector::new(
        TracingInspectorConfig::none()
            .set_steps(false)
            .set_record_inputs(true)
            .set_record_logs(true),
    );
    let _ = evm
        .transact_with_inspector(&evm2_transaction(case), &mut inspector)
        .map_err(|error| anyhow::anyhow!("evm2: {error:?}"))?;
    Ok(inspector.into_traces())
}

/// The call-level fields of a trace node that EDR reads.
#[derive(Debug, PartialEq, Eq)]
struct TraceNode {
    parent: Option<usize>,
    children: Vec<usize>,
    depth: usize,
    success: bool,
    caller: Address,
    address: Address,
    kind: String,
    value: U256,
    data: Bytes,
    output: Bytes,
    gas_used: u64,
    gas_limit: u64,
    gas_refund_counter: u64,
    status: Option<String>,
    selfdestruct: Option<(Option<Address>, Option<Address>, Option<U256>)>,
    logs: Vec<(Address, Vec<B256>, Bytes, u64)>,
}

fn revm_nodes(arena: &revm_inspectors::tracing::CallTraceArena) -> Vec<TraceNode> {
    arena
        .nodes()
        .iter()
        .map(|node| {
            let trace = &node.trace;
            TraceNode {
                parent: node.parent,
                children: node.children.clone(),
                depth: trace.depth,
                success: trace.success,
                caller: trace.caller,
                address: trace.address,
                kind: format!("{:?}", trace.kind),
                value: trace.value,
                data: trace.data.clone(),
                output: trace.output.clone(),
                gas_used: trace.gas_used,
                gas_limit: trace.gas_limit,
                gas_refund_counter: trace.gas_refund_counter,
                status: trace.status.map(|status| format!("{status:?}")),
                selfdestruct: trace.selfdestruct_address.map(|address| {
                    (
                        Some(address),
                        trace.selfdestruct_refund_target,
                        trace.selfdestruct_transferred_value,
                    )
                }),
                logs: node
                    .logs
                    .iter()
                    .map(|log| {
                        (
                            log.address,
                            log.raw_log.topics().to_vec(),
                            log.raw_log.data.clone(),
                            log.position,
                        )
                    })
                    .collect(),
            }
        })
        .collect()
}

fn evm2_nodes(arena: &evm2_inspectors::tracing::CallTraceArena) -> Vec<TraceNode> {
    arena
        .nodes()
        .iter()
        .map(|node| {
            let trace = &node.trace;
            TraceNode {
                parent: node.parent,
                children: node.children.clone(),
                depth: usize::from(trace.depth),
                success: trace.success,
                caller: trace.caller,
                address: trace.address,
                kind: format!("{:?}", trace.kind),
                value: trace.value,
                data: trace.data.clone(),
                output: trace.output.clone(),
                gas_used: trace.gas_used,
                gas_limit: trace.gas_limit,
                gas_refund_counter: trace.gas_refund_counter,
                status: trace.status.map(|status| format!("{status:?}")),
                selfdestruct: trace.selfdestruct.as_ref().map(|selfdestruct| {
                    (
                        selfdestruct.address,
                        selfdestruct.refund_target,
                        selfdestruct.transferred_value,
                    )
                }),
                logs: node
                    .logs
                    .iter()
                    .map(|log| {
                        (
                            log.address,
                            log.raw_log.topics().to_vec(),
                            log.raw_log.data.clone(),
                            log.position,
                        )
                    })
                    .collect(),
            }
        })
        .collect()
}

/// revm's state changes, as EDR commits them today.
fn revm_state_changes(state: &PersistentStateTrie, case: &Case) -> anyhow::Result<EvmState> {
    let execution = edr_evm::dry_run::<L1ChainSpec, _, _, _>(
        NoBlockHashes,
        state,
        CfgEnv::new_with_spec(case.fork.revm()),
        revm_transaction(case),
        revm_block(),
        &HashMap::default(),
    )
    .map_err(|error| anyhow::anyhow!("revm: {error}"))?;
    Ok(execution.state)
}

/// evm2's state changes, converted into what EDR's commit takes: the
/// adapter's job.
fn evm2_state_changes(state: &PersistentStateTrie, case: &Case) -> anyhow::Result<EvmState> {
    let code_loads = Cell::new(0);
    let code_conversion = Cell::new(Duration::ZERO);
    let mut evm = evm2_evm(state, case, &code_loads, &code_conversion);
    let executed = evm
        .transact(&evm2_transaction(case))
        .map_err(|error| anyhow::anyhow!("evm2: {error:?}"))?;

    let mut changes = EvmState::default();
    for change in executed.account_changes() {
        if !change.touched && !change.is_changed() {
            continue;
        }
        if change.storage.wiped && !change.created {
            println!("  note: {} storage wiped without creation", change.address);
        }

        let mut info = change
            .current
            .map_or_else(EdrAccountInfo::default, |current| EdrAccountInfo {
                balance: current.balance,
                nonce: current.nonce,
                code_hash: current.code_hash,
                ..EdrAccountInfo::default()
            });
        if let Some((code_hash, code)) = change.code {
            info.code_hash = code_hash;
            info.code = Some(RevmBytecode::new_raw(code.original_bytes()));
        }

        let mut account = Account::from(info);
        account.storage = change
            .storage
            .slots
            .iter()
            .map(|(key, slot)| {
                (
                    *key,
                    EvmStorageSlot::new_changed(
                        slot.value.original,
                        slot.value.current,
                        TransactionId::ZERO,
                    ),
                )
            })
            .collect();
        if change.touched {
            account.mark_touch();
        }
        if change.created {
            account.mark_created();
        }
        if change.selfdestructed {
            account.mark_selfdestruct();
        }
        changes.insert(change.address, account);
    }
    let _ = executed.discard();
    Ok(changes)
}

/// Commits `changes` into a copy of `state` with EDR's commit.
fn committed(state: &PersistentStateTrie, changes: EvmState) -> PersistentStateTrie {
    let mut state = state.clone();
    state.commit(changes);
    state
}

/// Compares two committed states; prints the first differing line of their
/// serialization.
fn same_state(revm: &PersistentStateTrie, evm2: &PersistentStateTrie) -> anyhow::Result<bool> {
    let equal = revm.state_root()? == evm2.state_root()?;
    if !equal {
        let (revm, evm2) = (revm.serialize(), evm2.serialize());
        for (revm, evm2) in revm.lines().zip(evm2.lines()) {
            if revm != evm2 {
                println!("  revm: {revm}\n  evm2: {evm2}");
            }
        }
    }
    Ok(equal)
}

/// Median of `MEASURED_RUNS` timings, after `WARMUP_RUNS`.
fn median_time(mut run: impl FnMut() -> anyhow::Result<()>) -> anyhow::Result<Duration> {
    for _ in 0..WARMUP_RUNS {
        run()?;
    }

    let mut timings = Vec::with_capacity(MEASURED_RUNS);
    for _ in 0..MEASURED_RUNS {
        let start = Instant::now();
        run()?;
        timings.push(start.elapsed());
    }

    timings.sort();
    Ok(timings[timings.len() / 2])
}

fn yes_no(equal: bool) -> &'static str {
    if equal {
        "ok"
    } else {
        "DIFF"
    }
}

fn main() -> anyhow::Result<()> {
    let state = edr_state()?;
    let cases = cases();
    let timing = std::env::args().any(|arg| arg == "--timing");

    println!("== Parity (revm 43 through EDR vs evm2 35aa328)");
    println!("case | fork | stop | outcome | steps | stacks | call traces");
    let mut differences = 0;
    for case in &cases {
        let mut revm_steps = RevmRecorder(Steps::new(true));
        let mut evm2_steps = Evm2Recorder(Steps::new(true));
        let revm = run_revm(&state, case, Some(&mut revm_steps))?;
        let evm2 = run_evm2(&state, case, Some(&mut evm2_steps))?;
        let revm_trace = revm_nodes(&revm_traces(&state, case)?);
        let evm2_trace = evm2_nodes(&evm2_traces(&state, case)?);

        let outcome_equal = revm == evm2.outcome;
        let steps_equal =
            revm_steps.0.pcs == evm2_steps.0.pcs && revm_steps.0.opcodes == evm2_steps.0.opcodes;
        let stacks_equal = revm_steps.0.stacks == evm2_steps.0.stacks;
        let traces_equal = revm_trace == evm2_trace;

        println!(
            "{} | {:?} | {} | {} | {} ({}) | {} | {} ({} nodes)",
            case.name,
            case.fork,
            evm2.outcome.stop,
            yes_no(outcome_equal),
            yes_no(steps_equal),
            revm_steps.0.pcs.len(),
            yes_no(stacks_equal),
            yes_no(traces_equal),
            revm_trace.len(),
        );
        if !outcome_equal {
            differences += 1;
            println!("  revm: {revm:?}\n  evm2: {:?}", evm2.outcome);
        }
        if !steps_equal {
            differences += 1;
            println!(
                "  steps: revm {} vs evm2 {}",
                revm_steps.0.pcs.len(),
                evm2_steps.0.pcs.len()
            );
        }
        if !traces_equal {
            differences += 1;
            if revm_trace.len() == evm2_trace.len() {
                for (index, (revm, evm2)) in revm_trace.iter().zip(&evm2_trace).enumerate() {
                    if revm != evm2 {
                        println!("  node {index}\n    revm: {revm:?}\n    evm2: {evm2:?}");
                    }
                }
            } else {
                println!("  revm nodes: {revm_trace:#?}\n  evm2 nodes: {evm2_trace:#?}");
            }
        }
        if evm2.code_loads > 0 && case.name == "loop contract" {
            println!(
                "  evm2 code loads: {}, conversion time: {:?}",
                evm2.code_loads, evm2.code_conversion
            );
        }
    }
    println!("\n{differences} difference(s) in {} cases", cases.len());

    println!("\n== Commit into EDR state (state root after EDR's commit)");
    let mut root_differences = 0;
    for case in &cases {
        let revm = committed(&state, revm_state_changes(&state, case)?);
        let evm2 = committed(&state, evm2_state_changes(&state, case)?);
        let equal = same_state(&revm, &evm2)?;
        if !equal {
            root_differences += 1;
        }
        println!("{} | {:?} | {}", case.name, case.fork, yes_no(equal));
        // Sanity: EIP-161 deletes touched empty accounts.
        for address in [EMPTY, EMPTY_DIRECT] {
            let before = state.basic(address)?.is_some();
            let (after_revm, after_evm2) = (
                revm.basic(address)?.is_some(),
                evm2.basic(address)?.is_some(),
            );
            if before != after_revm || before != after_evm2 {
                println!(
                    "  {address}: exists before {before}, revm {after_revm}, evm2 {after_evm2}"
                );
            }
        }
    }
    println!(
        "{root_differences} root difference(s) in {} cases",
        cases.len()
    );

    println!("\n== Chained: all cases in sequence, each engine on its own state");
    let (mut revm_state, mut evm2_state) = (state.clone(), state.clone());
    let mut chained_differences = 0;
    for case in &cases {
        let nonce = revm_state.basic(CALLER)?.map_or(0, |info| info.nonce);
        let case = Case {
            nonce,
            ..case.clone()
        };
        let revm_changes = revm_state_changes(&revm_state, &case)?;
        let evm2_changes = evm2_state_changes(&evm2_state, &case)?;
        revm_state.commit(revm_changes);
        evm2_state.commit(evm2_changes);
        if !same_state(&revm_state, &evm2_state)? {
            chained_differences += 1;
            println!("  after {} (nonce {nonce})", case.name);
        }
    }
    println!(
        "{chained_differences} root difference(s) over {} chained transactions, final root {}",
        cases.len(),
        revm_state.state_root()?
    );

    if !timing {
        return Ok(());
    }

    println!("\n== Timing (median of {MEASURED_RUNS} runs)");
    for case in cases.iter().take(2) {
        for (label, inspect, record_stack) in [
            ("no inspector", false, false),
            ("steps (pc, opcode)", true, false),
            ("steps + full stack", true, true),
        ] {
            let revm_time = median_time(|| {
                let mut recorder = RevmRecorder(Steps::new(record_stack));
                run_revm(&state, case, inspect.then_some(&mut recorder)).map(|_| ())
            })?;
            let evm2_time = median_time(|| {
                let mut recorder = Evm2Recorder(Steps::new(record_stack));
                run_evm2(&state, case, inspect.then_some(&mut recorder)).map(|_| ())
            })?;
            println!(
                "{} / {label}: revm {revm_time:?}, evm2 {evm2_time:?} (evm2/revm {:.2})",
                case.name,
                evm2_time.as_secs_f64() / revm_time.as_secs_f64()
            );
        }
    }

    Ok(())
}
