//! revm@41 ↔ revm@38 type conversions.
//!
//! Naming: `*_new_to_old` converts revm@41 → revm@38 (inbound, toward
//! op-revm); `*_old_to_new` converts revm@38 → revm@41 (outbound, results).
//!
//! Every conversion fully destructures its input (no `..` rest pattern), so
//! a field added or removed by a future revm version fails compilation
//! instead of being silently dropped.

use revm38::{
    context::{BlockEnv as BlockEnv38, CfgEnv as CfgEnv38, TxEnv as TxEnv38},
    context_interface::{
        block::BlobExcessGasAndPrice as BlobExcessGasAndPrice38,
        result::{
            ExecutionResult as ExecutionResult38, HaltReason as HaltReason38,
            OutOfGasError as OutOfGasError38, Output as Output38, ResultGas as ResultGas38,
            SuccessReason as SuccessReason38,
        },
    },
    state::{
        Account as Account38, AccountInfo as AccountInfo38, AccountStatus as AccountStatus38,
        EvmState as EvmState38, EvmStorageSlot as EvmStorageSlot38,
    },
};
use revm41::{
    context::{BlockEnv as BlockEnv41, CfgEnv as CfgEnv41, TxEnv as TxEnv41},
    context_interface::{
        block::BlobExcessGasAndPrice as BlobExcessGasAndPrice41,
        result::{
            ExecutionResult as ExecutionResult41, HaltReason as HaltReason41,
            OutOfGasError as OutOfGasError41, Output as Output41, ResultGas as ResultGas41,
            SuccessReason as SuccessReason41,
        },
    },
    state::{
        Account as Account41, AccountId as AccountId41, AccountInfo as AccountInfo41,
        AccountStatus as AccountStatus41, EvmState as EvmState41,
        EvmStorageSlot as EvmStorageSlot41, TransactionId as TransactionId41,
    },
};

pub type Bytecode38 = revm38::bytecode::Bytecode;
pub type Bytecode41 = revm41::bytecode::Bytecode;

// ---------------------------------------------------------------------------
// Inbound: revm@41 → revm@38
// ---------------------------------------------------------------------------

/// Both `CfgEnv`s are `#[non_exhaustive]`, so unlike the other conversions
/// this one cannot fully destructure: a field added by a future revm version
/// will NOT fail compilation here. Review this function on every revm bump.
///
/// `map_spec` bridges the two hardfork worlds: the input spec is EDR's
/// hardfork type (revm@41-compatible), the output is what op-revm expects
/// (e.g. `OpHardfork` → `OpSpecId`).
pub fn cfg_env_new_to_old<SpecInT, SpecOutT>(
    cfg: CfgEnv41<SpecInT>,
    map_spec: impl FnOnce(SpecInT) -> SpecOutT,
) -> CfgEnv38<SpecOutT>
where
    SpecOutT: Clone + Into<revm38::primitives::hardfork::SpecId>,
{
    let CfgEnv41 {
        spec,
        gas_params,
        chain_id,
        tx_chain_id_check,
        limit_contract_code_size,
        limit_contract_initcode_size,
        disable_nonce_check,
        max_blobs_per_tx,
        blob_base_fee_update_fraction,
        tx_gas_limit_cap,
        enable_amsterdam_eip8037,
        amsterdam_eip7708_disabled,
        amsterdam_eip7708_delayed_burn_disabled,
        ..
    } = cfg;

    // Amsterdam-era behavior has no revm@38 representation; refuse loudly
    // rather than silently dropping the flags.
    assert!(
        !enable_amsterdam_eip8037,
        "EIP-8037 state gas cannot be enabled on the op-revm (revm@38) side"
    );
    assert!(
        !amsterdam_eip7708_disabled && !amsterdam_eip7708_delayed_burn_disabled,
        "EIP-7708 overrides have no meaning on the op-revm (revm@38) side"
    );

    // Gas params are re-derived from the spec by `new_with_spec` rather than
    // copied: the raw `[u64; 256]` table is indexed by `GasId`, whose indices
    // may shift between revm majors, so copying would silently misassign
    // costs. Requires callers to never customize gas tables (EDR doesn't).
    let _ = gas_params;

    let mut out = CfgEnv38::new_with_spec(map_spec(spec));
    out.chain_id = chain_id;
    out.tx_chain_id_check = tx_chain_id_check;
    out.limit_contract_code_size = limit_contract_code_size;
    out.limit_contract_initcode_size = limit_contract_initcode_size;
    out.disable_nonce_check = disable_nonce_check;
    out.max_blobs_per_tx = max_blobs_per_tx;
    out.blob_base_fee_update_fraction = blob_base_fee_update_fraction;
    out.tx_gas_limit_cap = tx_gas_limit_cap;
    out
}

pub fn block_env_new_to_old(block: BlockEnv41) -> BlockEnv38 {
    let BlockEnv41 {
        number,
        beneficiary,
        timestamp,
        gas_limit,
        basefee,
        difficulty,
        prevrandao,
        blob_excess_gas_and_price,
        slot_num,
    } = block;

    BlockEnv38 {
        number,
        beneficiary,
        timestamp,
        gas_limit,
        basefee,
        difficulty,
        prevrandao,
        blob_excess_gas_and_price: blob_excess_gas_and_price
            .map(blob_excess_gas_and_price_new_to_old),
        slot_num,
    }
}

fn blob_excess_gas_and_price_new_to_old(blob: BlobExcessGasAndPrice41) -> BlobExcessGasAndPrice38 {
    let BlobExcessGasAndPrice41 {
        excess_blob_gas,
        blob_gasprice,
    } = blob;
    BlobExcessGasAndPrice38 {
        excess_blob_gas,
        blob_gasprice,
    }
}

pub fn tx_env_new_to_old(tx: TxEnv41) -> TxEnv38 {
    let TxEnv41 {
        tx_type,
        caller,
        gas_limit,
        gas_price,
        kind,
        value,
        data,
        nonce,
        chain_id,
        access_list,
        gas_priority_fee,
        blob_hashes,
        max_fee_per_blob_gas,
        authorization_list,
    } = tx;

    TxEnv38 {
        tx_type,
        caller,
        gas_limit,
        gas_price,
        kind,
        value,
        data,
        nonce,
        chain_id,
        access_list,
        gas_priority_fee,
        blob_hashes,
        max_fee_per_blob_gas,
        authorization_list,
    }
}

pub fn account_info_new_to_old(info: AccountInfo41) -> AccountInfo38 {
    let AccountInfo41 {
        balance,
        nonce,
        code_hash,
        account_id,
        code,
    } = info;

    AccountInfo38 {
        balance,
        nonce,
        code_hash,
        account_id: account_id.map(AccountId41::get),
        code: code.map(bytecode_new_to_old),
    }
}

/// Re-analyzes from the original bytes: jump tables are recomputed on the
/// target side, so only the code itself crosses the boundary.
pub fn bytecode_new_to_old(code: Bytecode41) -> Bytecode38 {
    Bytecode38::new_raw(code.original_bytes())
}

// ---------------------------------------------------------------------------
// Outbound: revm@38 → revm@41
// ---------------------------------------------------------------------------

pub fn bytecode_old_to_new(code: Bytecode38) -> Bytecode41 {
    Bytecode41::new_raw(code.original_bytes())
}

pub fn account_info_old_to_new(info: AccountInfo38) -> AccountInfo41 {
    let AccountInfo38 {
        balance,
        nonce,
        code_hash,
        account_id,
        code,
    } = info;

    AccountInfo41 {
        balance,
        nonce,
        code_hash,
        account_id: account_id
            .map(|id| AccountId41::new(id).expect("account id exceeds revm@41 representation")),
        code: code.map(bytecode_old_to_new),
    }
}

pub fn account_status_old_to_new(status: AccountStatus38) -> AccountStatus41 {
    AccountStatus41::from_bits(status.bits())
        .expect("revm@38 account status bits unknown to revm@41")
}

pub fn storage_slot_old_to_new(slot: EvmStorageSlot38) -> EvmStorageSlot41 {
    let EvmStorageSlot38 {
        original_value,
        present_value,
        transaction_id,
        is_cold,
    } = slot;

    EvmStorageSlot41 {
        original_value,
        present_value,
        transaction_id: transaction_id_old_to_new(transaction_id),
        is_cold,
    }
}

fn transaction_id_old_to_new(transaction_id: usize) -> TransactionId41 {
    TransactionId41::new(transaction_id).expect("transaction id exceeds revm@41 representation")
}

pub fn account_old_to_new(account: Account38) -> Account41 {
    let Account38 {
        info,
        original_info,
        transaction_id,
        storage,
        status,
    } = account;

    let mut out = Account41::default();
    out.info = account_info_old_to_new(info);
    *out.original_info_mut() = account_info_old_to_new(*original_info);
    out.transaction_id = transaction_id_old_to_new(transaction_id);
    for (key, slot) in storage {
        out.storage.insert(key, storage_slot_old_to_new(slot));
    }
    out.status = account_status_old_to_new(status);
    out
}

pub fn state_old_to_new(state: EvmState38) -> EvmState41 {
    let mut out = EvmState41::default();
    for (address, account) in state {
        out.insert(address, account_old_to_new(account));
    }
    out
}

/// Halt reason type is generic (e.g. `OpHaltReason`, a single op-revm type on
/// both sides here) and passes through unchanged; the base-reason mapping EDR
/// needs is [`halt_reason_old_to_new`].
pub fn execution_result_old_to_new<HaltReasonT>(
    result: ExecutionResult38<HaltReasonT>,
) -> ExecutionResult41<HaltReasonT> {
    match result {
        ExecutionResult38::Success {
            reason,
            gas,
            logs,
            output,
        } => ExecutionResult41::Success {
            reason: success_reason_old_to_new(reason),
            gas: result_gas_old_to_new(gas),
            logs,
            output: output_old_to_new(output),
        },
        ExecutionResult38::Revert { gas, logs, output } => ExecutionResult41::Revert {
            gas: result_gas_old_to_new(gas),
            logs,
            output,
        },
        ExecutionResult38::Halt { reason, gas, logs } => ExecutionResult41::Halt {
            reason,
            gas: result_gas_old_to_new(gas),
            logs,
        },
    }
}

/// The new side documents `state_gas_spent` as net of the EIP-7702
/// per-authorization refund; the old side doesn't apply that refund. op-revm
/// runs pre-EIP-8037 specs where state gas is always zero, so the difference
/// is unobservable — asserted rather than assumed.
fn result_gas_old_to_new(gas: ResultGas38) -> ResultGas41 {
    assert_eq!(
        gas.state_gas_spent(),
        0,
        "non-zero revm@38 state gas cannot be faithfully converted to revm@41"
    );
    ResultGas41::new_with_state_gas(
        gas.total_gas_spent(),
        gas.inner_refunded(),
        gas.floor_gas(),
        0,
    )
}

fn success_reason_old_to_new(reason: SuccessReason38) -> SuccessReason41 {
    match reason {
        SuccessReason38::Stop => SuccessReason41::Stop,
        SuccessReason38::Return => SuccessReason41::Return,
        SuccessReason38::SelfDestruct => SuccessReason41::SelfDestruct,
    }
}

fn output_old_to_new(output: Output38) -> Output41 {
    match output {
        Output38::Call(bytes) => Output41::Call(bytes),
        Output38::Create(bytes, address) => Output41::Create(bytes, address),
    }
}

pub fn halt_reason_old_to_new(reason: HaltReason38) -> HaltReason41 {
    match reason {
        HaltReason38::OutOfGas(error) => HaltReason41::OutOfGas(out_of_gas_error_old_to_new(error)),
        HaltReason38::OpcodeNotFound => HaltReason41::OpcodeNotFound,
        HaltReason38::InvalidFEOpcode => HaltReason41::InvalidFEOpcode,
        HaltReason38::InvalidJump => HaltReason41::InvalidJump,
        HaltReason38::NotActivated => HaltReason41::NotActivated,
        HaltReason38::StackUnderflow => HaltReason41::StackUnderflow,
        HaltReason38::StackOverflow => HaltReason41::StackOverflow,
        HaltReason38::OutOfOffset => HaltReason41::OutOfOffset,
        HaltReason38::CreateCollision => HaltReason41::CreateCollision,
        HaltReason38::PrecompileError => HaltReason41::PrecompileError,
        HaltReason38::PrecompileErrorWithContext(message) => {
            HaltReason41::PrecompileErrorWithContext(message)
        }
        HaltReason38::NonceOverflow => HaltReason41::NonceOverflow,
        HaltReason38::CreateContractSizeLimit => HaltReason41::CreateContractSizeLimit,
        HaltReason38::CreateContractStartingWithEF => HaltReason41::CreateContractStartingWithEF,
        HaltReason38::CreateInitCodeSizeLimit => HaltReason41::CreateInitCodeSizeLimit,
        HaltReason38::OverflowPayment => HaltReason41::OverflowPayment,
        HaltReason38::StateChangeDuringStaticCall => HaltReason41::StateChangeDuringStaticCall,
        HaltReason38::CallNotAllowedInsideStatic => HaltReason41::CallNotAllowedInsideStatic,
        HaltReason38::OutOfFunds => HaltReason41::OutOfFunds,
        HaltReason38::CallTooDeep => HaltReason41::CallTooDeep,
    }
}

fn out_of_gas_error_old_to_new(error: OutOfGasError38) -> OutOfGasError41 {
    match error {
        OutOfGasError38::Basic => OutOfGasError41::Basic,
        OutOfGasError38::MemoryLimit => OutOfGasError41::MemoryLimit,
        OutOfGasError38::Memory => OutOfGasError41::Memory,
        OutOfGasError38::Precompile => OutOfGasError41::Precompile,
        OutOfGasError38::InvalidOperand => OutOfGasError41::InvalidOperand,
        OutOfGasError38::ReentrancySentry => OutOfGasError41::ReentrancySentry,
    }
}
