//! An evm2 adapter for L1 that takes and returns the types EDR uses with revm
//! today: `CfgEnv`, a revm `Block`, `L1SignedTransaction`, revm `PrecompileFn`s
//! in, and `ExecutionResult` + `EvmState` (or a `TransactionError`) out.
//!
//! [`dry_run`] has the same inputs and outputs as `edr_evm::dry_run`, so both
//! engines can be compared with `==`.

use alloy_consensus::{
    transaction::Recovered, TxEip1559, TxEip2930, TxEip4844, TxEip4844Variant, TxEip7702,
    TxLegacy,
};
use edr_blockchain_api::BlockHashByNumber;
use edr_chain_l1::{HaltReason, InvalidTransaction, L1Hardfork, L1SignedTransaction};
use edr_chain_spec::{BlockEnvTrait, EvmSpecId, ExecutableTransaction, OutOfGasError};
use edr_chain_spec_evm::{
    result::{ExecutionResult, Output, ResultGas, SuccessReason},
    CfgEnv, TransactionError,
};
use edr_database_components::DatabaseComponentError;
use edr_evm::ExecutionResultAndStateWithMetadata;
use edr_precompile::PrecompileFn;
use edr_primitives::{Address, Bytecode as RevmBytecode, HashMap, HashSet, B256, U256};
use edr_state_api::{
    account::{Account, AccountInfo as EdrAccountInfo},
    EvmState, EvmStorageSlot, State, TransactionId,
};
use evm2::{
    bytecode::Bytecode as Evm2Bytecode,
    env::BlockEnvExt,
    ethereum::{ethereum_tx_registry, TxEnvelope},
    evm::{
        precompile::{PrecompileOutput as Evm2PrecompileOutput, PrecompileProvider},
        Database, Db, PrewarmSet,
    },
    interpreter::{GasTracker, InstrStop, Message},
    precompiles::{
        MovePrecompileError, PrecompileError as Evm2PrecompileError, PrecompileHalt,
        PrecompileId,
    },
    registry::HandlerError,
    AnyError, BaseEvmTypes, Evm, EvmFeatures, ExecutionConfig, Precompiles, Version,
};
use revm_interpreter::InstructionResult;
use std::{cell::RefCell, rc::Rc};

/// EDR's hardfork to evm2's spec, through the revm spec EDR already maps to.
pub fn spec_id(hardfork: L1Hardfork) -> evm2::SpecId {
    match EvmSpecId::from(hardfork) {
        EvmSpecId::FRONTIER => evm2::SpecId::FRONTIER,
        EvmSpecId::HOMESTEAD => evm2::SpecId::HOMESTEAD,
        EvmSpecId::TANGERINE => evm2::SpecId::TANGERINE,
        EvmSpecId::SPURIOUS_DRAGON => evm2::SpecId::SPURIOUS_DRAGON,
        EvmSpecId::BYZANTIUM => evm2::SpecId::BYZANTIUM,
        EvmSpecId::PETERSBURG => evm2::SpecId::PETERSBURG,
        EvmSpecId::ISTANBUL => evm2::SpecId::ISTANBUL,
        EvmSpecId::BERLIN => evm2::SpecId::BERLIN,
        EvmSpecId::LONDON => evm2::SpecId::LONDON,
        EvmSpecId::MERGE => evm2::SpecId::MERGE,
        EvmSpecId::SHANGHAI => evm2::SpecId::SHANGHAI,
        EvmSpecId::CANCUN => evm2::SpecId::CANCUN,
        EvmSpecId::PRAGUE => evm2::SpecId::PRAGUE,
        EvmSpecId::OSAKA => evm2::SpecId::OSAKA,
        EvmSpecId::AMSTERDAM => evm2::SpecId::AMSTERDAM,
    }
}

/// `CfgEnv` to evm2's runtime version: limits, caps and the checks EDR turns
/// off.
pub fn version(cfg: &CfgEnv<L1Hardfork>) -> Version {
    let spec = spec_id(cfg.spec);
    let mut version = Version::new(spec);
    version.chain_id = cfg.chain_id;
    version.memory_limit = cfg.memory_limit;
    if let Some(cap) = cfg.tx_gas_limit_cap {
        version.tx_gas_limit_cap = cap;
    }
    if let Some(limit) = cfg.limit_contract_code_size {
        version.max_code_size = limit;
        version.max_initcode_size = limit.saturating_mul(2);
    }
    if let Some(limit) = cfg.limit_contract_initcode_size {
        version.max_initcode_size = limit;
    }
    // revm checks the per-transaction blob count only when it is configured.
    version.max_blobs_per_tx = cfg
        .max_blobs_per_tx
        .map_or(usize::MAX, |max| usize::try_from(max).expect("fits"));

    for (disabled, feature) in [
        (!cfg.tx_chain_id_check, EvmFeatures::TX_CHAIN_ID_CHECK),
        (cfg.disable_nonce_check, EvmFeatures::NONCE_CHECK),
        (cfg.disable_balance_check, EvmFeatures::BALANCE_CHECK),
        (cfg.disable_block_gas_limit, EvmFeatures::BLOCK_GAS_LIMIT_CHECK),
        (cfg.disable_eip3541, EvmFeatures::EIP3541),
        (cfg.disable_eip3607, EvmFeatures::EIP3607),
        (cfg.disable_eip7623, EvmFeatures::EIP7623),
        (cfg.disable_base_fee, EvmFeatures::BASE_FEE_CHECK),
    ] {
        if disabled {
            version.features.remove(feature);
        }
    }

    version
}

/// A revm `Block` (EDR's `HeaderAndEvmSpec` implements it) to evm2's block
/// env.
pub fn block_env(block: &impl BlockEnvTrait) -> BlockEnvExt {
    BlockEnvExt {
        number: block.number(),
        beneficiary: block.beneficiary(),
        timestamp: block.timestamp(),
        gas_limit: U256::from(block.gas_limit()),
        basefee: U256::from(block.basefee()),
        difficulty: block.difficulty(),
        prevrandao: block
            .prevrandao()
            .map_or(U256::ZERO, |prevrandao| U256::from_be_bytes(prevrandao.0)),
        blob_basefee: U256::from(block.blob_gasprice().unwrap_or_default()),
        slot_num: U256::from(block.slot_num()),
        ..BlockEnvExt::default()
    }
}

/// EDR's signed L1 transaction to evm2's recovered envelope, through EDR's
/// `ExecutableTransaction` trait.
pub fn transaction(transaction: &L1SignedTransaction) -> Recovered<TxEnvelope> {
    let tx = transaction;
    let access_list = || tx.access_list().unwrap_or_default().to_vec().into();
    let max_fee = || *tx.max_fee_per_gas().expect("post-London transaction");
    let max_priority_fee = || {
        *tx.max_priority_fee_per_gas()
            .expect("post-London transaction")
    };
    let to = || match tx.kind() {
        alloy_primitives::TxKind::Call(to) => to,
        alloy_primitives::TxKind::Create => unreachable!("type 3 and 4 transactions can't create"),
    };

    let envelope = match transaction {
        L1SignedTransaction::PreEip155Legacy(_) | L1SignedTransaction::PostEip155Legacy(_) => {
            TxEnvelope::Legacy(TxLegacy {
                chain_id: tx.chain_id(),
                nonce: tx.nonce(),
                gas_price: *tx.gas_price(),
                gas_limit: tx.gas_limit(),
                to: tx.kind(),
                value: *tx.value(),
                input: tx.data().clone(),
            })
        }
        L1SignedTransaction::Eip2930(_) => TxEnvelope::Eip2930(TxEip2930 {
            chain_id: tx.chain_id().expect("typed transaction"),
            nonce: tx.nonce(),
            gas_price: *tx.gas_price(),
            gas_limit: tx.gas_limit(),
            to: tx.kind(),
            value: *tx.value(),
            access_list: access_list(),
            input: tx.data().clone(),
        }),
        L1SignedTransaction::Eip1559(_) => TxEnvelope::Eip1559(TxEip1559 {
            chain_id: tx.chain_id().expect("typed transaction"),
            nonce: tx.nonce(),
            gas_limit: tx.gas_limit(),
            max_fee_per_gas: max_fee(),
            max_priority_fee_per_gas: max_priority_fee(),
            to: tx.kind(),
            value: *tx.value(),
            access_list: access_list(),
            input: tx.data().clone(),
        }),
        L1SignedTransaction::Eip4844(_) => {
            TxEnvelope::Eip4844(TxEip4844Variant::TxEip4844(TxEip4844 {
                chain_id: tx.chain_id().expect("typed transaction"),
                nonce: tx.nonce(),
                gas_limit: tx.gas_limit(),
                max_fee_per_gas: max_fee(),
                max_priority_fee_per_gas: max_priority_fee(),
                to: to(),
                value: *tx.value(),
                access_list: access_list(),
                blob_versioned_hashes: tx.blob_hashes().to_vec(),
                max_fee_per_blob_gas: *tx.max_fee_per_blob_gas().expect("blob transaction"),
                input: tx.data().clone(),
            }))
        }
        L1SignedTransaction::Eip7702(_) => TxEnvelope::Eip7702(
            TxEip7702 {
                chain_id: tx.chain_id().expect("typed transaction"),
                nonce: tx.nonce(),
                gas_limit: tx.gas_limit(),
                max_fee_per_gas: max_fee(),
                max_priority_fee_per_gas: max_priority_fee(),
                to: to(),
                value: *tx.value(),
                access_list: access_list(),
                authorization_list: tx
                    .authorization_list()
                    .expect("set-code transaction")
                    .to_vec(),
                input: tx.data().clone(),
            }
            .into(),
        ),
    };

    Recovered::new_unchecked(envelope, *tx.caller())
}

/// evm2's halt to revm's halt reason.
pub fn halt_reason(stop: InstrStop) -> HaltReason {
    match stop {
        InstrStop::OutOfGas => HaltReason::OutOfGas(OutOfGasError::Basic),
        InstrStop::MemoryOOG => HaltReason::OutOfGas(OutOfGasError::Memory),
        InstrStop::MemoryLimitOOG => HaltReason::OutOfGas(OutOfGasError::MemoryLimit),
        InstrStop::PrecompileOOG => HaltReason::OutOfGas(OutOfGasError::Precompile),
        InstrStop::InvalidOperandOOG => HaltReason::OutOfGas(OutOfGasError::InvalidOperand),
        InstrStop::ReentrancySentryOOG => HaltReason::OutOfGas(OutOfGasError::ReentrancySentry),
        InstrStop::OpcodeNotFound => HaltReason::OpcodeNotFound,
        InstrStop::InvalidFEOpcode => HaltReason::InvalidFEOpcode,
        InstrStop::InvalidJump => HaltReason::InvalidJump,
        InstrStop::NotActivated => HaltReason::NotActivated,
        InstrStop::StackUnderflow => HaltReason::StackUnderflow,
        InstrStop::StackOverflow => HaltReason::StackOverflow,
        InstrStop::OutOfOffset => HaltReason::OutOfOffset,
        InstrStop::CreateCollision => HaltReason::CreateCollision,
        InstrStop::PrecompileError => HaltReason::PrecompileError,
        InstrStop::NonceOverflow => HaltReason::NonceOverflow,
        InstrStop::CreateContractSizeLimit => HaltReason::CreateContractSizeLimit,
        InstrStop::CreateContractStartingWithEF => HaltReason::CreateContractStartingWithEF,
        InstrStop::CreateInitCodeSizeLimit => HaltReason::CreateInitCodeSizeLimit,
        InstrStop::CallNotAllowedInsideStatic => HaltReason::CallNotAllowedInsideStatic,
        InstrStop::StateChangeDuringStaticCall => HaltReason::StateChangeDuringStaticCall,
        InstrStop::OutOfFunds => HaltReason::OutOfFunds,
        InstrStop::CallTooDeep => HaltReason::CallTooDeep,
        other => unreachable!("not a transaction halt: {other:?}"),
    }
}

/// evm2's transaction result to revm's `ExecutionResult`.
pub fn execution_result(result: &evm2::TxResultExt, is_create: bool) -> ExecutionResult<HaltReason> {
    let gas = ResultGas::new_with_state_gas(
        result.total_gas_spent,
        result.refunded,
        result.floor_gas,
        result.state_gas_spent,
    );
    let logs = result.logs.clone();

    if result.is_success() {
        let output = if is_create {
            Output::Create(result.output.clone(), result.created_address)
        } else {
            Output::Call(result.output.clone())
        };
        // revm reports every successful create as `Return`: the frame ends by
        // depositing code, whatever stopped the init code.
        let reason = match result.stop {
            _ if is_create => SuccessReason::Return,
            InstrStop::Stop => SuccessReason::Stop,
            InstrStop::Return => SuccessReason::Return,
            InstrStop::SelfDestruct => SuccessReason::SelfDestruct,
            other => unreachable!("not a success: {other:?}"),
        };
        ExecutionResult::Success {
            reason,
            gas,
            logs,
            output,
        }
    } else if result.is_revert() {
        ExecutionResult::Revert {
            gas,
            logs,
            output: result.output.clone(),
        }
    } else {
        ExecutionResult::Halt {
            reason: halt_reason(result.stop),
            gas,
            logs,
        }
    }
}

/// evm2's validation errors to revm's `InvalidTransaction`, inside EDR's
/// `TransactionError`.
pub fn transaction_error<DatabaseErrorT>(
    error: HandlerError,
) -> TransactionError<DatabaseErrorT, InvalidTransaction> {
    let invalid = match error {
        HandlerError::Database(error) => return TransactionError::Custom(error.to_string()),
        HandlerError::Fatal(error) | HandlerError::External(error) => {
            return TransactionError::CustomAny(edr_chain_spec_evm::result::AnyError::new(
                StringError(error.to_string()),
            ))
        }
        HandlerError::UnsupportedTransactionType(1)
        | HandlerError::WrongTransactionType { expected: 1 } => {
            InvalidTransaction::Eip2930NotSupported
        }
        HandlerError::UnsupportedTransactionType(2)
        | HandlerError::WrongTransactionType { expected: 2 } => {
            InvalidTransaction::Eip1559NotSupported
        }
        HandlerError::UnsupportedTransactionType(3)
        | HandlerError::WrongTransactionType { expected: 3 } => {
            InvalidTransaction::Eip4844NotSupported
        }
        HandlerError::UnsupportedTransactionType(4)
        | HandlerError::WrongTransactionType { expected: 4 } => {
            InvalidTransaction::Eip7702NotSupported
        }
        HandlerError::UnsupportedTransactionType(other)
        | HandlerError::WrongTransactionType { expected: other } => {
            InvalidTransaction::Str(format!("unsupported transaction type {other}").into())
        }
        HandlerError::InvalidNonce { expected, got } if got > expected => {
            InvalidTransaction::NonceTooHigh {
                tx: got,
                state: expected,
            }
        }
        HandlerError::InvalidNonce { expected, got } => InvalidTransaction::NonceTooLow {
            tx: got,
            state: expected,
        },
        HandlerError::InvalidChainId { .. } => InvalidTransaction::InvalidChainId,
        HandlerError::MissingChainId => InvalidTransaction::MissingChainId,
        HandlerError::IntrinsicGasTooLow { required, got } => {
            InvalidTransaction::CallGasCostMoreThanGasLimit {
                initial_gas: required,
                gas_limit: got,
            }
        }
        HandlerError::GasLimitBelowFloor { required, got } => {
            InvalidTransaction::GasFloorMoreThanGasLimit {
                gas_floor: required,
                gas_limit: got,
            }
        }
        HandlerError::InsufficientFunds { fee, balance } => {
            InvalidTransaction::LackOfFundForMaxFee {
                fee: Box::new(fee),
                balance: Box::new(balance),
            }
        }
        HandlerError::RejectCallerWithCode => InvalidTransaction::RejectCallerWithCode,
        HandlerError::NonceOverflow => InvalidTransaction::NonceOverflowInTransaction,
        HandlerError::GasLimitMoreThanBlock { .. } => {
            InvalidTransaction::CallerGasLimitMoreThanBlock
        }
        HandlerError::TxGasLimitGreaterThanCap { gas_limit, cap } => {
            InvalidTransaction::TxGasLimitGreaterThanCap { gas_limit, cap }
        }
        HandlerError::IntrinsicGasGreaterThanCap { required, cap } => InvalidTransaction::Str(
            format!("intrinsic gas {required} exceeds cap {cap}").into(),
        ),
        HandlerError::CreateInitCodeSizeLimit { .. } => {
            InvalidTransaction::CreateInitCodeSizeLimit
        }
        HandlerError::FeeCapLessThanBaseFee { .. } => InvalidTransaction::GasPriceLessThanBasefee,
        HandlerError::EmptyAuthorizationList => InvalidTransaction::EmptyAuthorizationList,
        HandlerError::BlobFeeCapLessThanBlobBaseFee {
            max_fee_per_blob_gas,
            blob_base_fee,
        } => InvalidTransaction::BlobGasPriceGreaterThanMax {
            block_blob_gas_price: blob_base_fee.to(),
            tx_max_fee_per_blob_gas: max_fee_per_blob_gas.to(),
        },
        HandlerError::EmptyBlobs => InvalidTransaction::EmptyBlobs,
        HandlerError::TooManyBlobs { have, max } => InvalidTransaction::TooManyBlobs { max, have },
        HandlerError::BlobVersionNotSupported => InvalidTransaction::BlobVersionNotSupported,
        HandlerError::PriorityFeeGreaterThanMaxFee => {
            InvalidTransaction::PriorityFeeGreaterThanMaxFee
        }
    };
    invalid.into()
}

#[derive(Debug)]
struct StringError(String);

impl std::fmt::Display for StringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StringError {}

/// evm2's account changes to the `EvmState` EDR's commit takes.
pub fn state_changes(executed: &evm2::ExecutedTx<'_, '_, BaseEvmTypes>) -> EvmState {
    let mut changes = EvmState::default();
    for change in executed.account_changes() {
        if !change.touched && !change.is_changed() {
            continue;
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
    changes
}

/// evm2 database over EDR's blockchain and state components.
struct ComponentsDatabase<BlockchainT, StateT> {
    blockchain: BlockchainT,
    state: StateT,
}

impl<BlockchainT, StateT> Database for ComponentsDatabase<BlockchainT, StateT>
where
    BlockchainT: BlockHashByNumber<Error: 'static + std::error::Error + Send + Sync>,
    StateT: State<Error: 'static + std::error::Error + Send + Sync>,
{
    type Error = DatabaseComponentError<BlockchainT::Error, StateT::Error>;

    fn get_account(&mut self, address: &Address) -> Result<Option<evm2::AccountInfo>, Self::Error> {
        Ok(self
            .state
            .basic(*address)
            .map_err(DatabaseComponentError::State)?
            .map(|info| evm2::AccountInfo::new(info.balance, info.nonce, info.code_hash, None)))
    }

    fn get_code_by_hash(&mut self, code_hash: &B256) -> Result<Evm2Bytecode, Self::Error> {
        let code = self
            .state
            .code_by_hash(*code_hash)
            .map_err(DatabaseComponentError::State)?;
        Ok(Evm2Bytecode::new_raw(code.original_bytes()))
    }

    fn get_storage(&mut self, address: &Address, key: &U256) -> Result<U256, Self::Error> {
        self.state
            .storage(*address, *key)
            .map_err(DatabaseComponentError::State)
    }

    fn get_block_hash(&mut self, number: &U256) -> Result<B256, Self::Error> {
        self.blockchain
            .block_hash_by_number(number.to())
            .map_err(DatabaseComponentError::Blockchain)
    }
}

/// evm2 precompiles for a spec plus EDR's custom precompiles, which keep
/// revm's `PrecompileFn` signature.
pub struct CustomPrecompiles<'custom> {
    base: Precompiles,
    custom: &'custom HashMap<Address, PrecompileFn>,
    /// The halt message of a top-level precompile call, which revm reports as
    /// `PrecompileErrorWithContext` and evm2 drops.
    top_level_halt: Rc<RefCell<Option<String>>>,
}

impl<'custom> CustomPrecompiles<'custom> {
    pub fn new(spec: evm2::SpecId, custom: &'custom HashMap<Address, PrecompileFn>) -> Self {
        Self {
            base: Precompiles::base(spec),
            custom,
            top_level_halt: Rc::default(),
        }
    }

    pub fn top_level_halt(&self) -> Rc<RefCell<Option<String>>> {
        self.top_level_halt.clone()
    }

    pub fn addresses(&self) -> HashSet<Address> {
        PrecompileProvider::<BaseEvmTypes>::addresses(&self.base)
            .into_iter()
            .chain(self.custom.keys().copied())
            .collect()
    }
}

impl PrecompileProvider<BaseEvmTypes> for CustomPrecompiles<'_> {
    fn addresses(&self) -> Vec<Address> {
        CustomPrecompiles::addresses(self).into_iter().collect()
    }

    fn warm_addresses(&self, warm: &mut PrewarmSet) {
        PrecompileProvider::<BaseEvmTypes>::warm_addresses(&self.base, warm);
        for address in self.custom.keys() {
            warm.warm(address);
        }
    }

    fn precompile_ids(&self) -> Vec<(Address, PrecompileId)> {
        let mut ids: Vec<_> = PrecompileProvider::<BaseEvmTypes>::precompile_ids(&self.base)
            .into_iter()
            .filter(|(address, _)| !self.custom.contains_key(address))
            .collect();
        ids.extend(
            self.custom
                .keys()
                .map(|address| (*address, PrecompileId::Custom("edr custom".into()))),
        );
        ids
    }

    fn contains(&self, address: &Address) -> bool {
        self.custom.contains_key(address)
            || PrecompileProvider::<BaseEvmTypes>::contains(&self.base, address)
    }

    fn move_precompiles(
        &mut self,
        moves: &[(Address, Address)],
    ) -> Result<(), MovePrecompileError> {
        PrecompileProvider::<BaseEvmTypes>::move_precompiles(&mut self.base, moves)
    }

    fn execute(
        &mut self,
        evm: &mut Evm<'_, BaseEvmTypes>,
        message: &Message<BaseEvmTypes>,
        gas: &mut GasTracker,
    ) -> Option<Result<Evm2PrecompileOutput, Evm2PrecompileError>> {
        let result = self.execute_inner(evm, message, gas);
        if let Some(Err(Evm2PrecompileError::Halt(halt))) = &result
            && *halt != PrecompileHalt::OutOfGas
            && message.depth == TOP_LEVEL_DEPTH
        {
            *self.top_level_halt.borrow_mut() = Some(halt.to_string());
        }
        result
    }
}

/// evm2's depth of the transaction's own frame.
const TOP_LEVEL_DEPTH: u16 = 0;

impl CustomPrecompiles<'_> {
    fn execute_inner(
        &mut self,
        evm: &mut Evm<'_, BaseEvmTypes>,
        message: &Message<BaseEvmTypes>,
        gas: &mut GasTracker,
    ) -> Option<Result<Evm2PrecompileOutput, Evm2PrecompileError>> {
        let Some(precompile) = self.custom.get(&message.call_target) else {
            return self.base.execute(evm, message, gas);
        };

        let output = match precompile(evm.message_input(message), gas.remaining(), gas.reservoir())
        {
            Ok(output) => output,
            Err(error) => {
                return Some(Err(Evm2PrecompileError::Fatal(AnyError::from(
                    error.to_string(),
                ))));
            }
        };

        let result = if output.status.is_halt() {
            let halt = match output.halt_reason() {
                Some(reason) if reason.is_oog() => PrecompileHalt::OutOfGas,
                Some(reason) => PrecompileHalt::Other(format!("{reason:?}").into()),
                None => unreachable!("halted"),
            };
            Err(Evm2PrecompileError::Halt(halt))
        } else if let Err(stop) = gas.spend(output.gas_used) {
            let _ = stop;
            Err(Evm2PrecompileError::Halt(PrecompileHalt::OutOfGas))
        } else if output.status.is_revert() {
            Err(Evm2PrecompileError::Revert(output.bytes))
        } else {
            Ok(Evm2PrecompileOutput::new(output.bytes))
        };
        Some(result)
    }
}

fn new_evm<'a>(
    database: impl Database + 'a,
    cfg: &CfgEnv<L1Hardfork>,
    block: &impl BlockEnvTrait,
    precompiles: CustomPrecompiles<'a>,
) -> Evm<'a, BaseEvmTypes> {
    let spec = spec_id(cfg.spec);
    Evm::<BaseEvmTypes>::new_with_execution_config(
        ExecutionConfig::for_spec_and_version(spec, version(cfg)),
        spec,
        block_env(block),
        ethereum_tx_registry(spec),
        Db::new(database),
        precompiles,
    )
}

/// Same inputs and outputs as `edr_evm::dry_run::<L1ChainSpec, ..>`.
#[allow(clippy::type_complexity)]
pub fn dry_run<BlockchainT, StateT>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<L1Hardfork>,
    transaction: L1SignedTransaction,
    block: impl BlockEnvTrait,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> Result<
    ExecutionResultAndStateWithMetadata<HaltReason>,
    TransactionError<DatabaseComponentError<BlockchainT::Error, StateT::Error>, InvalidTransaction>,
>
where
    BlockchainT: BlockHashByNumber<Error: 'static + std::error::Error + Send + Sync>,
    StateT: State<Error: 'static + std::error::Error + Send + Sync>,
{
    let precompiles = CustomPrecompiles::new(spec_id(cfg.spec), custom_precompiles);
    let precompile_addresses = precompiles.addresses();
    let top_level_halt = precompiles.top_level_halt();
    let mut evm = new_evm(
        ComponentsDatabase { blockchain, state },
        &cfg,
        &block,
        precompiles,
    );

    let is_create = transaction.kind().is_create();
    let executed = evm
        .transact(&self::transaction(&transaction))
        .map_err(transaction_error)?;
    let state = state_changes(&executed);
    let mut result = execution_result(&executed.discard(), is_create);
    if let ExecutionResult::Halt { reason, .. } = &mut result
        && *reason == HaltReason::PrecompileError
        && let Some(message) = top_level_halt.borrow_mut().take()
    {
        *reason = HaltReason::PrecompileErrorWithContext(message);
    }

    Ok(ExecutionResultAndStateWithMetadata {
        precompile_addresses,
        result,
        state,
    })
}

/// Runs evm2-inspectors' tracer with the config of EDR's
/// `SolidityTracingInspector` and returns revm-inspectors' arena.
pub fn call_trace<BlockchainT, StateT>(
    blockchain: BlockchainT,
    state: StateT,
    cfg: CfgEnv<L1Hardfork>,
    transaction: L1SignedTransaction,
    block: impl BlockEnvTrait,
    custom_precompiles: &HashMap<Address, PrecompileFn>,
) -> anyhow::Result<revm_inspectors::tracing::CallTraceArena>
where
    BlockchainT: BlockHashByNumber<Error: 'static + std::error::Error + Send + Sync>,
    StateT: State<Error: 'static + std::error::Error + Send + Sync>,
{
    use evm2_inspectors::tracing::{TracingInspector, TracingInspectorConfig};

    let precompiles = CustomPrecompiles::new(spec_id(cfg.spec), custom_precompiles);
    let mut evm = new_evm(
        ComponentsDatabase { blockchain, state },
        &cfg,
        &block,
        precompiles,
    );
    let mut inspector = TracingInspector::new(
        TracingInspectorConfig::none()
            .set_steps(false)
            .set_record_inputs(true)
            .set_record_logs(true),
    );
    let _ = evm
        .transact_with_inspector(&self::transaction(&transaction), &mut inspector)
        .map_err(|error| anyhow::anyhow!("evm2: {error:?}"))?
        .discard();
    let mut arena = call_trace_arena(inspector.into_traces());
    revm_compatible(&mut arena);
    Ok(arena)
}

/// Aligns the two trace differences that the arena alone can bridge with what
/// revm-inspectors reports.
pub fn revm_compatible(arena: &mut revm_inspectors::tracing::CallTraceArena) {
    use revm_inspectors::tracing::types::CallKind;

    let created: HashSet<Address> = arena
        .nodes()
        .iter()
        .filter(|node| matches!(node.trace.kind, CallKind::Create | CallKind::Create2))
        .map(|node| node.trace.address)
        .collect();
    for node in arena.nodes_mut() {
        let trace = &mut node.trace;
        // A successful create ends by depositing code: revm reports `Return`.
        if matches!(trace.kind, CallKind::Create | CallKind::Create2) && trace.success {
            trace.status = Some(InstructionResult::Return);
        }
        // revm records no selfdestruct when an existing contract names itself
        // as the beneficiary.
        if trace.selfdestruct_refund_target == Some(trace.address)
            && !created.contains(&trace.address)
        {
            trace.selfdestruct_address = None;
            trace.selfdestruct_refund_target = None;
            trace.selfdestruct_transferred_value = None;
        }
    }
}

/// evm2-inspectors' arena to revm-inspectors' arena, field by field.
pub fn call_trace_arena(
    arena: evm2_inspectors::tracing::CallTraceArena,
) -> revm_inspectors::tracing::CallTraceArena {
    use evm2_inspectors::tracing::types as from;
    use revm_inspectors::tracing::types as to;

    let mut converted = revm_inspectors::tracing::CallTraceArena::default();
    let nodes = converted.nodes_mut();
    nodes.clear();
    for node in arena.into_nodes() {
        let trace = node.trace;
        let (selfdestruct_address, selfdestruct_refund_target, selfdestruct_transferred_value) =
            trace.selfdestruct.map_or((None, None, None), |selfdestruct| {
                (
                    selfdestruct.address,
                    selfdestruct.refund_target,
                    selfdestruct.transferred_value,
                )
            });
        nodes.push(to::CallTraceNode {
            parent: node.parent,
            children: node.children,
            idx: node.idx,
            trace: to::CallTrace {
                depth: usize::from(trace.depth),
                success: trace.success,
                caller: trace.caller,
                address: trace.address,
                maybe_precompile: trace.maybe_precompile,
                selfdestruct_address,
                selfdestruct_refund_target,
                selfdestruct_transferred_value,
                kind: match trace.kind {
                    from::CallKind::Call => to::CallKind::Call,
                    from::CallKind::StaticCall => to::CallKind::StaticCall,
                    from::CallKind::CallCode => to::CallKind::CallCode,
                    from::CallKind::DelegateCall => to::CallKind::DelegateCall,
                    from::CallKind::AuthCall => to::CallKind::AuthCall,
                    from::CallKind::Create => to::CallKind::Create,
                    from::CallKind::Create2 => to::CallKind::Create2,
                },
                value: trace.value,
                data: trace.data,
                output: trace.output,
                bytecode: trace.bytecode,
                gas_used: trace.gas_used,
                gas_limit: trace.gas_limit,
                gas_refund_counter: trace.gas_refund_counter,
                status: trace.status.map(instruction_result),
                steps: Vec::new(),
                step_deltas: Vec::new(),
                decoded: None,
            },
            logs: node
                .logs
                .into_iter()
                .map(|log| to::CallLog {
                    address: log.address,
                    raw_log: log.raw_log,
                    decoded: None,
                    position: log.position,
                    index: log.index,
                })
                .collect(),
            ordering: node
                .ordering
                .into_iter()
                .map(|member| match member {
                    from::TraceMemberOrder::Log(index) => to::TraceMemberOrder::Log(index),
                    from::TraceMemberOrder::Call(index) => to::TraceMemberOrder::Call(index),
                    from::TraceMemberOrder::Step(index) => to::TraceMemberOrder::Step(index),
                })
                .collect(),
        });
    }
    converted
}

fn instruction_result(stop: InstrStop) -> InstructionResult {
    match stop {
        InstrStop::Stop => InstructionResult::Stop,
        InstrStop::Return => InstructionResult::Return,
        InstrStop::SelfDestruct => InstructionResult::SelfDestruct,
        InstrStop::Revert => InstructionResult::Revert,
        InstrStop::CallTooDeep => InstructionResult::CallTooDeep,
        InstrStop::OutOfFunds => InstructionResult::OutOfFunds,
        InstrStop::NonceOverflow => InstructionResult::NonceOverflow,
        InstrStop::OutOfGas => InstructionResult::OutOfGas,
        InstrStop::MemoryOOG => InstructionResult::MemoryOOG,
        InstrStop::MemoryLimitOOG => InstructionResult::MemoryLimitOOG,
        InstrStop::PrecompileOOG => InstructionResult::PrecompileOOG,
        InstrStop::InvalidOperandOOG => InstructionResult::InvalidOperandOOG,
        InstrStop::ReentrancySentryOOG => InstructionResult::ReentrancySentryOOG,
        InstrStop::CallNotAllowedInsideStatic => InstructionResult::CallNotAllowedInsideStatic,
        InstrStop::StateChangeDuringStaticCall => InstructionResult::StateChangeDuringStaticCall,
        InstrStop::OpcodeNotFound => InstructionResult::OpcodeNotFound,
        InstrStop::InvalidFEOpcode => InstructionResult::InvalidFEOpcode,
        InstrStop::InvalidJump => InstructionResult::InvalidJump,
        InstrStop::NotActivated => InstructionResult::NotActivated,
        InstrStop::StackUnderflow => InstructionResult::StackUnderflow,
        InstrStop::StackOverflow => InstructionResult::StackOverflow,
        InstrStop::OutOfOffset => InstructionResult::OutOfOffset,
        InstrStop::CreateCollision => InstructionResult::CreateCollision,
        InstrStop::PrecompileError => InstructionResult::PrecompileError,
        InstrStop::CreateContractSizeLimit => InstructionResult::CreateContractSizeLimit,
        InstrStop::CreateContractStartingWithEF => InstructionResult::CreateContractStartingWithEF,
        InstrStop::CreateInitCodeSizeLimit => InstructionResult::CreateInitCodeSizeLimit,
        InstrStop::FatalExternalError => InstructionResult::FatalExternalError,
        InstrStop::InvalidImmediateEncoding => InstructionResult::InvalidImmediateEncoding,
        other => unreachable!("internal stop: {other:?}"),
    }
}
