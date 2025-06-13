//! Implementations of [`Evm`](crate::Group::Evm) cheatcodes.

use std::{
    collections::{btree_map::Entry, BTreeMap, HashMap},
    fmt::Display,
    path::Path,
};

use alloy_genesis::{Genesis, GenesisAccount};
use alloy_primitives::{Address, Bytes, B256, U256};
use alloy_sol_types::SolValue;
use edr_common::fs::{read_json_file, write_json_file};
use foundry_evm_core::{
    backend::{CheatcodeBackend, RevertSnapshotAction},
    constants::{CALLER, CHEATCODE_ADDRESS, HARDHAT_CONSOLE_ADDRESS, TEST_CONTRACT_ADDRESS},
    evm_context::{
        split_context, BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
    },
};
use revm::{
    bytecode::Bytecode,
    context::{result::HaltReasonTr, CfgEnv, JournalTr},
    primitives::{hardfork::SpecId, KECCAK_EMPTY},
    state::Account,
    Journal,
};
use serde::Serialize;
use spec::Vm::signCall;

use crate::{
    impl_is_pure_false, impl_is_pure_true, Cheatcode, Cheatcodes, CheatsCtxt, Result,
    Vm::{
        accessesCall, addrCall, blobBaseFeeCall, blobhashesCall, chainIdCall, coinbaseCall,
        coolCall, dealCall, deleteSnapshotCall, deleteSnapshotsCall, difficultyCall, dumpStateCall,
        etchCall, feeCall, getBlobBaseFeeCall, getBlobhashesCall, getBlockNumberCall,
        getBlockTimestampCall, getNonceCall, getRecordedLogsCall, lastCallGasCall, loadAllocsCall,
        loadCall, pauseGasMeteringCall, prevrandao_0Call, prevrandao_1Call, readCallersCall,
        recordCall, recordLogsCall, resetNonceCall, resumeGasMeteringCall, revertToAndDeleteCall,
        revertToCall, rollCall, setNonceCall, setNonceUnsafeCall, signP256Call, snapshotCall,
        startStateDiffRecordingCall, stopAndReturnStateDiffCall, getStateDiffCall, getStateDiffJsonCall, storeCall, txGasPriceCall,
        warpCall, CallerMode,
    },
};

mod fork;
pub(crate) mod mapping;
pub(crate) mod mock;
pub(crate) mod prank;

/// Records storage slots reads and writes.
#[derive(Clone, Debug, Default)]
pub struct RecordAccess {
    /// Storage slots reads.
    pub reads: HashMap<Address, Vec<U256>>,
    /// Storage slots writes.
    pub writes: HashMap<Address, Vec<U256>>,
}

/// Records `deal` cheatcodes
#[derive(Clone, Debug)]
pub struct DealRecord {
    /// Target of the deal.
    pub address: Address,
    /// The balance of the address before deal was applied
    pub old_balance: U256,
    /// Balance after deal was applied
    pub new_balance: U256,
}

/// Storage slot diff info.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct SlotStateDiff {
    /// Initial storage value.
    previous_value: B256,
    /// Current storage value.
    new_value: B256,
}

/// Balance diff info.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct BalanceDiff {
    /// Initial storage value.
    previous_value: U256,
    /// Current storage value.
    new_value: U256,
}

/// Account state diff info.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct AccountStateDiffs {
    /// Address label, if any set.
    label: Option<String>,
    /// Account balance changes.
    balance_diff: Option<BalanceDiff>,
    /// State changes, per slot.
    state_diff: BTreeMap<B256, SlotStateDiff>,
}

impl Display for AccountStateDiffs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print changed account.
        if let Some(label) = &self.label {
            writeln!(f, "label: {label}")?;
        }
        // Print balance diff if changed.
        if let Some(balance_diff) = &self.balance_diff {
            if balance_diff.previous_value != balance_diff.new_value {
                writeln!(
                    f,
                    "- balance diff: {} → {}",
                    balance_diff.previous_value, balance_diff.new_value
                )?;
            }
        }
        // Print state diff if any.
        if !&self.state_diff.is_empty() {
            writeln!(f, "- state diff:")?;
            for (slot, slot_changes) in &self.state_diff {
                writeln!(
                    f,
                    "@ {slot}: {} → {}",
                    slot_changes.previous_value, slot_changes.new_value
                )?;
            }
        }

        Ok(())
    }
}

impl_is_pure_true!(addrCall);
impl Cheatcode for addrCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    >(
        &self,
        _state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Result {
        let Self { privateKey } = self;
        let wallet = super::utils::parse_wallet(privateKey)?;
        Ok(wallet.address().abi_encode())
    }
}

impl_is_pure_true!(getNonceCall);
impl Cheatcode for getNonceCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { account } = self;
        get_nonce(ccx, account)
    }
}

impl_is_pure_true!(loadCall);
impl Cheatcode for loadCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { target, slot } = *self;
        ensure_not_precompile!(&target, ccx);
        ccx.ecx.journaled_state.load_account(target)?;
        let val = ccx.ecx.journaled_state.sload(target, slot.into())?;
        Ok(val.abi_encode())
    }
}

impl_is_pure_false!(loadAllocsCall);
impl Cheatcode for loadAllocsCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { pathToAllocsJson } = self;

        let path = Path::new(pathToAllocsJson);
        ensure!(
            path.exists(),
            "allocs file does not exist: {pathToAllocsJson}"
        );

        // Let's first assume we're reading a file with only the allocs.

        let allocs: BTreeMap<Address, GenesisAccount> = if let Ok(allocs) = read_json_file(path) {
            allocs
        } else {
            // Let's try and read from a genesis file, and extract allocs.
            let genesis = read_json_file::<Genesis>(path)?;
            genesis.alloc
        };

        // Then, load the allocs into the database.
        let (db, context) = split_context(ccx.ecx);
        db.load_allocs(&allocs, context.journaled_state)
            .map(|()| Vec::default())
            .map_err(|e| fmt_err!("failed to load allocs: {e}"))
    }
}

impl_is_pure_false!(dumpStateCall);
impl Cheatcode for dumpStateCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { pathToStateJson } = self;
        let path = Path::new(pathToStateJson);

        // Do not include system account or empty accounts in the dump.
        let skip = |key: &Address, val: &Account| {
            key == &CHEATCODE_ADDRESS
                || key == &CALLER
                || key == &HARDHAT_CONSOLE_ADDRESS
                || key == &TEST_CONTRACT_ADDRESS
                || key == &ccx.caller
                || key == &ccx.state.config.evm_opts.sender
                || val.is_empty()
        };

        let alloc = ccx
            .ecx
            .journaled_state
            .state()
            .iter_mut()
            .filter(|(key, val)| !skip(key, val))
            .map(|(key, val)| {
                (
                    key,
                    GenesisAccount {
                        nonce: Some(val.info.nonce),
                        balance: val.info.balance,
                        code: val.info.code.as_ref().map(Bytecode::original_bytes),
                        storage: Some(
                            val.storage
                                .iter()
                                .map(|(k, v)| (B256::from(*k), B256::from(v.present_value())))
                                .collect(),
                        ),
                        private_key: None,
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        write_json_file(path, &alloc)?;
        Ok(Vec::default())
    }
}

impl_is_pure_true!(signCall);
impl Cheatcode for signCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        _: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { privateKey, digest } = self;
        super::utils::sign(privateKey, digest)
    }
}

impl_is_pure_true!(signP256Call);
impl Cheatcode for signP256Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        _ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { privateKey, digest } = self;
        super::utils::sign_p256(privateKey, digest)
    }
}

impl_is_pure_true!(recordCall);
impl Cheatcode for recordCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    >(
        &self,
        state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Result {
        let Self {} = self;
        state.accesses = Some(RecordAccess::default());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(accessesCall);
impl Cheatcode for accessesCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    >(
        &self,
        state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Result {
        let Self { target } = *self;
        let result = state
            .accesses
            .as_mut()
            .map(|accesses| {
                (
                    &accesses.reads.entry(target).or_default()[..],
                    &accesses.writes.entry(target).or_default()[..],
                )
            })
            .unwrap_or_default();
        Ok(result.abi_encode_params())
    }
}

impl_is_pure_true!(recordLogsCall);
impl Cheatcode for recordLogsCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    >(
        &self,
        state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Result {
        let Self {} = self;
        state.recorded_logs = Some(Vec::default());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(getRecordedLogsCall);
impl Cheatcode for getRecordedLogsCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    >(
        &self,
        state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Result {
        let Self {} = self;
        Ok(state
            .recorded_logs
            .replace(Vec::default())
            .unwrap_or_default()
            .abi_encode())
    }
}

impl_is_pure_true!(pauseGasMeteringCall);
impl Cheatcode for pauseGasMeteringCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    >(
        &self,
        state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Result {
        let Self {} = self;
        if state.gas_metering.is_none() {
            state.gas_metering = Some(None);
        }
        Ok(Vec::default())
    }
}

impl_is_pure_true!(resumeGasMeteringCall);
impl Cheatcode for resumeGasMeteringCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    >(
        &self,
        state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Result {
        let Self {} = self;
        state.gas_metering = None;
        Ok(Vec::default())
    }
}

impl_is_pure_true!(lastCallGasCall);
impl Cheatcode for lastCallGasCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    >(
        &self,
        state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Result {
        let Self {} = self;
        ensure!(
            state.last_call_gas.is_some(),
            "`lastCallGas` is only available after a call"
        );
        Ok(state
            .last_call_gas
            .as_ref()
            // This should never happen, as we ensure `last_call_gas` is `Some` above.
            .expect("`lastCallGas` is only available after a call")
            .abi_encode())
    }
}

impl_is_pure_true!(chainIdCall);
impl Cheatcode for chainIdCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { newChainId } = self;
        ensure!(
            *newChainId <= U256::from(u64::MAX),
            "chain ID must be less than 2^64 - 1"
        );
        ccx.ecx.cfg.chain_id = newChainId.to();
        Ok(Vec::default())
    }
}

impl_is_pure_true!(coinbaseCall);
impl Cheatcode for coinbaseCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { newCoinbase } = self;
        ccx.ecx.block.set_beneficiary(*newCoinbase);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(difficultyCall);
impl Cheatcode for difficultyCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { newDifficulty } = self;
        ensure!(
            ccx.ecx.cfg.spec.into() < SpecId::MERGE,
            "`difficulty` is not supported after the Paris hard fork, use `prevrandao` instead; \
             see EIP-4399: https://eips.ethereum.org/EIPS/eip-4399"
        );
        ccx.ecx.block.set_difficulty(*newDifficulty);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(feeCall);
impl Cheatcode for feeCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { newBasefee } = self;
        ensure!(
            *newBasefee <= U256::from(u64::MAX),
            "base fee must be less than 2^64 - 1"
        );
        ccx.ecx.block.set_basefee(newBasefee.saturating_to());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(prevrandao_0Call);
impl Cheatcode for prevrandao_0Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { newPrevrandao } = self;
        ensure!(
            ccx.ecx.cfg.spec.into() >= SpecId::MERGE,
            "`prevrandao` is not supported before the Paris hard fork, use `difficulty` instead; \
             see EIP-4399: https://eips.ethereum.org/EIPS/eip-4399"
        );
        ccx.ecx.block.set_prevrandao(*newPrevrandao);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(prevrandao_1Call);
impl Cheatcode for prevrandao_1Call {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { newPrevrandao } = self;
        ensure!(
            ccx.ecx.cfg.spec.into() >= SpecId::MERGE,
            "`prevrandao` is not supported before the Paris hard fork, use `difficulty` instead; \
             see EIP-4399: https://eips.ethereum.org/EIPS/eip-4399"
        );
        ccx.ecx.block.set_prevrandao((*newPrevrandao).into());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(blobhashesCall);
impl Cheatcode for blobhashesCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { hashes } = self;
        ensure!(
            ccx.ecx.cfg.spec.into() >= SpecId::CANCUN,
            "`blobhash` is not supported before the Cancun hard fork; \
             see EIP-4844: https://eips.ethereum.org/EIPS/eip-4844"
        );
        ccx.ecx.tx.set_blob_versioned_hashes(hashes.clone());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(getBlobhashesCall);
impl Cheatcode for getBlobhashesCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {} = self;
        ensure!(
            ccx.ecx.cfg.spec.into() >= SpecId::CANCUN,
            "`blobhash` is not supported before the Cancun hard fork; \
             see EIP-4844: https://eips.ethereum.org/EIPS/eip-4844"
        );
        Ok(ccx.ecx.tx.blob_versioned_hashes().abi_encode())
    }
}

impl_is_pure_true!(rollCall);
impl Cheatcode for rollCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { newHeight } = self;
        ensure!(
            *newHeight <= U256::from(u64::MAX),
            "block height must be less than 2^64 - 1"
        );
        ccx.ecx.block.set_block_number(newHeight.saturating_to());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(getBlockNumberCall);
impl Cheatcode for getBlockNumberCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {} = self;
        Ok(ccx.ecx.block.number().abi_encode())
    }
}

impl_is_pure_true!(txGasPriceCall);
impl Cheatcode for txGasPriceCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { newGasPrice } = self;
        ensure!(
            *newGasPrice <= U256::from(u128::MAX),
            "gas price must be less than 2^128 - 1"
        );
        ccx.ecx.tx.set_gas_price(newGasPrice.saturating_to());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(warpCall);
impl Cheatcode for warpCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { newTimestamp } = self;
        ensure!(
            *newTimestamp <= U256::from(u64::MAX),
            "timestamp must be less than 2^64 - 1"
        );
        ccx.ecx.block.set_timestamp(newTimestamp.saturating_to());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(getBlockTimestampCall);
impl Cheatcode for getBlockTimestampCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {} = self;
        Ok(ccx.ecx.block.timestamp().abi_encode())
    }
}

impl_is_pure_true!(blobBaseFeeCall);
impl Cheatcode for blobBaseFeeCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { newBlobBaseFee } = self;
        let spec_id: SpecId = ccx.ecx.cfg.spec.into();
        ensure!(
            spec_id >= SpecId::CANCUN,
            "`blobBaseFee` is not supported before the Cancun hard fork; \
             see EIP-4844: https://eips.ethereum.org/EIPS/eip-4844"
        );
        ccx.ecx
            .block
            .set_blob_excess_gas_and_price((*newBlobBaseFee).to(), spec_id >= SpecId::PRAGUE);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(getBlobBaseFeeCall);
impl Cheatcode for getBlobBaseFeeCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {} = self;
        Ok(ccx.ecx.block.blob_excess_gas().unwrap_or(0).abi_encode())
    }
}

impl_is_pure_true!(dealCall);
impl Cheatcode for dealCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            account: address,
            newBalance: new_balance,
        } = *self;
        let account = journaled_account(ccx.ecx, address)?;
        let old_balance = std::mem::replace(&mut account.info.balance, new_balance);
        let record = DealRecord {
            address,
            old_balance,
            new_balance,
        };
        ccx.state.eth_deals.push(record);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(etchCall);
impl Cheatcode for etchCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            target,
            newRuntimeBytecode,
        } = self;
        ensure_not_precompile!(target, ccx);
        ccx.ecx.journaled_state.load_account(*target)?;
        let bytecode = Bytecode::new_raw(Bytes::copy_from_slice(newRuntimeBytecode));
        ccx.ecx.journaled_state.set_code(*target, bytecode);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(resetNonceCall);
impl Cheatcode for resetNonceCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { account } = self;
        let account = journaled_account(ccx.ecx, *account)?;
        // Per EIP-161, EOA nonces start at 0, but contract nonces
        // start at 1. Comparing by code_hash instead of code
        // to avoid hitting the case where account's code is None.
        let empty = account.info.code_hash == KECCAK_EMPTY;
        let nonce = u64::from(!empty);
        account.info.nonce = nonce;
        debug!(target: "cheatcodes", nonce, "reset");
        Ok(Vec::default())
    }
}

impl_is_pure_true!(setNonceCall);
impl Cheatcode for setNonceCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { account, newNonce } = *self;
        let account = journaled_account(ccx.ecx, account)?;
        // nonce must increment only
        let current = account.info.nonce;
        ensure!(
            newNonce >= current,
            "new nonce ({newNonce}) must be strictly equal to or higher than the \
             account's current nonce ({current})"
        );
        account.info.nonce = newNonce;
        Ok(Vec::default())
    }
}

impl_is_pure_true!(setNonceUnsafeCall);
impl Cheatcode for setNonceUnsafeCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { account, newNonce } = *self;
        let account = journaled_account(ccx.ecx, account)?;
        account.info.nonce = newNonce;
        Ok(Vec::default())
    }
}

impl_is_pure_true!(storeCall);
impl Cheatcode for storeCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            target,
            slot,
            value,
        } = *self;
        ensure_not_precompile!(&target, ccx);
        // ensure the account is touched
        let _ = journaled_account(ccx.ecx, target)?;
        ccx.ecx
            .journaled_state
            .sstore(target, slot.into(), value.into())?;
        Ok(Vec::default())
    }
}

impl_is_pure_true!(coolCall);
impl Cheatcode for coolCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { target } = self;
        if let Some(account) = ccx.ecx.journaled_state.state.get_mut(target) {
            account.unmark_touch();
            account.storage.clear();
        }
        Ok(Vec::default())
    }
}

impl_is_pure_true!(readCallersCall);
impl Cheatcode for readCallersCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {} = self;
        read_callers(ccx.state, &ccx.ecx.tx.caller())
    }
}

impl_is_pure_true!(snapshotCall);
impl Cheatcode for snapshotCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {} = self;
        let (db, context) = split_context(ccx.ecx);
        Ok(db
            .snapshot(context.journaled_state, context.to_owned_env())
            .abi_encode())
    }
}

impl_is_pure_true!(revertToCall);
impl Cheatcode for revertToCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { snapshotId } = self;
        let (db, mut context) = split_context(ccx.ecx);
        let result = if let Some(journaled_state) =
            db.revert(*snapshotId, RevertSnapshotAction::RevertKeep, &mut context)
        {
            // we reset the evm's journaled_state to the state of the snapshot previous
            // state
            ccx.ecx.journaled_state.inner = journaled_state;
            true
        } else {
            false
        };
        Ok(result.abi_encode())
    }
}

impl_is_pure_true!(revertToAndDeleteCall);
impl Cheatcode for revertToAndDeleteCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { snapshotId } = self;
        let (db, mut context) = split_context(ccx.ecx);
        let result = if let Some(journaled_state) = db.revert(
            *snapshotId,
            RevertSnapshotAction::RevertRemove,
            &mut context,
        ) {
            // we reset the evm's journaled_state to the state of the snapshot previous
            // state
            ccx.ecx.journaled_state.inner = journaled_state;
            true
        } else {
            false
        };
        Ok(result.abi_encode())
    }
}

impl_is_pure_true!(deleteSnapshotCall);
impl Cheatcode for deleteSnapshotCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { snapshotId } = self;
        let result = ccx
            .ecx
            .journaled_state
            .database
            .delete_snapshot(*snapshotId);
        Ok(result.abi_encode())
    }
}

impl_is_pure_true!(deleteSnapshotsCall);
impl Cheatcode for deleteSnapshotsCall {
    fn apply_full<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {} = self;
        ccx.ecx.journaled_state.database.delete_snapshots();
        Ok(Vec::default())
    }
}

impl_is_pure_true!(startStateDiffRecordingCall);
impl Cheatcode for startStateDiffRecordingCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    >(
        &self,
        state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Result {
        let Self {} = self;
        state.recorded_account_diffs_stack = Some(Vec::default());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(stopAndReturnStateDiffCall);
impl Cheatcode for stopAndReturnStateDiffCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    >(
        &self,
        state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Result {
        let Self {} = self;
        get_state_diff(state)
    }
}

impl_is_pure_true!(getStateDiffCall);
impl Cheatcode for getStateDiffCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    >(
        &self,
        state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Result {
        let Self {} = self;
        let mut diffs = String::new();
        let state_diffs = get_recorded_state_diffs(state);
        for (address, state_diffs) in state_diffs {
            diffs.push_str(&format!("{address}\n"));
            diffs.push_str(&format!("{state_diffs}\n"));
        }
        Ok(diffs.abi_encode())
    }
}

impl_is_pure_true!(getStateDiffJsonCall);
impl Cheatcode for getStateDiffJsonCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
    >(
        &self,
        state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    ) -> Result {
        let Self {} = self;
        let state_diffs = get_recorded_state_diffs(state);
        Ok(serde_json::to_string(&state_diffs)?.abi_encode())
    }
}

pub(super) fn get_nonce<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    ChainContextT: ChainContextTr,
    DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
>(
    ccx: &mut CheatsCtxt<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        ChainContextT,
        DatabaseT,
    >,
    address: &Address,
) -> Result {
    let account = ccx.ecx.journaled_state.load_account(*address)?;
    Ok(account.info.nonce.abi_encode())
}

/// Reads the current caller information and returns the current [`CallerMode`],
/// `msg.sender` and `tx.origin`.
///
/// Depending on the current caller mode, one of the following results will be
/// returned:
/// - If there is an active prank:
///     - `caller_mode` will be equal to:
///         - [`CallerMode::Prank`] if the prank has been set with
///           `vm.prank(..)`.
///         - [`CallerMode::RecurrentPrank`] if the prank has been set with
///           `vm.startPrank(..)`.
///     - `msg.sender` will be equal to the address set for the prank.
///     - `tx.origin` will be equal to the default sender address unless an
///       alternative one has been set when configuring the prank.
///
/// - If no caller modification is active:
///     - `caller_mode` will be equal to [`CallerMode::None`],
///     - `msg.sender` and `tx.origin` will be equal to the default sender
///       address.
fn read_callers<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
>(
    state: &Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
    default_sender: &Address,
) -> Result {
    let Cheatcodes { prank, .. } = state;

    let mut mode = CallerMode::None;
    let mut new_caller = default_sender;
    let mut new_origin = default_sender;
    if let Some(prank) = prank {
        mode = if prank.single_call {
            CallerMode::Prank
        } else {
            CallerMode::RecurrentPrank
        };
        new_caller = &prank.new_caller;
        if let Some(new) = &prank.new_origin {
            new_origin = new;
        }
    }

    Ok((mode, new_caller, new_origin).abi_encode_params())
}

/// Ensures the `Account` is loaded and touched.
pub(super) fn journaled_account<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    ChainContextT: ChainContextTr,
    DatabaseT: CheatcodeBackend<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT>,
>(
    ecx: &mut revm::context::Context<
        BlockT,
        TxT,
        CfgEnv<HardforkT>,
        DatabaseT,
        Journal<DatabaseT>,
        ChainContextT,
    >,
    addr: Address,
) -> Result<&mut Account> {
    ecx.journaled_state.load_account(addr)?;
    ecx.journaled_state.touch(addr);
    Ok(ecx
        .journaled_state
        .state
        .get_mut(&addr)
        .expect("account is loaded"))
}

/// Consumes recorded account accesses and returns them as an abi encoded
/// array of [`AccountAccess`]. If there are no accounts were
/// recorded as accessed, an abi encoded empty array is returned.
///
/// In the case where `stopAndReturnStateDiff` is called at a lower
/// depth than `startStateDiffRecording`, multiple
/// `Vec<RecordedAccountAccesses>` will be flattened, preserving the order of
/// the accesses.
fn get_state_diff<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
>(
    state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
) -> Result {
    let res = state
        .recorded_account_diffs_stack
        .replace(Vec::default())
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    Ok(res.abi_encode())
}

/// Helper function to returns state diffs recorded for each changed account.
fn get_recorded_state_diffs<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
>(
    state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT>,
) -> BTreeMap<Address, AccountStateDiffs> {
    let mut state_diffs: BTreeMap<Address, AccountStateDiffs> = BTreeMap::default();
    if let Some(records) = &state.recorded_account_diffs_stack {
        records
            .iter()
            .flatten()
            .filter(|account_access| {
                !account_access.storageAccesses.is_empty() ||
                    account_access.oldBalance != account_access.newBalance
            })
            .for_each(|account_access| {
                let account_diff =
                    state_diffs.entry(account_access.account).or_insert(AccountStateDiffs {
                        label: state.labels.get(&account_access.account).cloned(),
                        ..Default::default()
                    });

                // Record account balance diffs.
                if account_access.oldBalance != account_access.newBalance {
                    // Update balance diff. Do not overwrite the initial balance if already set.
                    if let Some(diff) = &mut account_diff.balance_diff {
                        diff.new_value = account_access.newBalance;
                    } else {
                        account_diff.balance_diff = Some(BalanceDiff {
                            previous_value: account_access.oldBalance,
                            new_value: account_access.newBalance,
                        });
                    }
                }

                // Record account state diffs.
                for storage_access in &account_access.storageAccesses {
                    if storage_access.isWrite && !storage_access.reverted {
                        // Update state diff. Do not overwrite the initial value if already set.
                        match account_diff.state_diff.entry(storage_access.slot) {
                            Entry::Vacant(slot_state_diff) => {
                                slot_state_diff.insert(SlotStateDiff {
                                    previous_value: storage_access.previousValue,
                                    new_value: storage_access.newValue,
                                });
                            }
                            Entry::Occupied(mut slot_state_diff) => {
                                slot_state_diff.get_mut().new_value = storage_access.newValue;
                            }
                        }
                    }
                }
            });
    }
    state_diffs
}
