use crate::{impl_is_pure_true, Cheatcode, Cheatcodes, CheatsCtxt, Error, Result};
use alloy_primitives::Address;
use foundry_evm_core::constants::MAGIC_ASSUME;
use spec::Vm::{
    PotentialRevert, assumeCall, assumeNoRevert_0Call, assumeNoRevert_1Call, assumeNoRevert_2Call,
};
use std::fmt::Debug;
use revm::context::result::HaltReasonTr;
use foundry_evm_core::backend::CheatcodeBackend;
use foundry_evm_core::evm_context::{BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr, TransactionErrorTrait};

#[derive(Clone, Debug)]
pub struct AssumeNoRevert {
    /// The call depth at which the cheatcode was added.
    pub depth: usize,
    /// Acceptable revert parameters for the next call, to be thrown out if they are encountered;
    /// reverts with parameters not specified here will count as normal reverts and not rejects
    /// towards the counter.
    pub reasons: Vec<AcceptableRevertParameters>,
    /// Address that reverted the call.
    pub reverted_by: Option<Address>,
}

/// Parameters for a single anticipated revert, to be thrown out if encountered.
#[derive(Clone, Debug)]
pub struct AcceptableRevertParameters {
    /// The expected revert data returned by the revert
    pub reason: Vec<u8>,
    /// If true then only the first 4 bytes of expected data returned by the revert are checked.
    pub partial_match: bool,
    /// Contract expected to revert next call.
    pub reverter: Option<Address>,
}

impl AcceptableRevertParameters {
    fn from(potential_revert: &PotentialRevert) -> Self {
        Self {
            reason: potential_revert.revertData.to_vec(),
            partial_match: potential_revert.partialMatch,
            reverter: if potential_revert.reverter == Address::ZERO {
                None
            } else {
                Some(potential_revert.reverter)
            },
        }
    }
}

impl_is_pure_true!(assumeCall);
impl Cheatcode for assumeCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(&self, _state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>) -> Result {
        let Self { condition } = self;
        if *condition { Ok(Vec::default()) } else { Err(Error::from(MAGIC_ASSUME)) }
    }
}

impl_is_pure_true!(assumeNoRevert_0Call);
impl Cheatcode for assumeNoRevert_0Call {
    fn apply_stateful<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(&self, ccx: &mut CheatsCtxt<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT, DatabaseT>) -> Result {
        assume_no_revert::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(ccx.state, ccx.ecx.journaled_state.depth, vec![])
    }
}

impl_is_pure_true!(assumeNoRevert_1Call);
impl Cheatcode for assumeNoRevert_1Call {
    fn apply_stateful<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(&self, ccx: &mut CheatsCtxt<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT, DatabaseT>) -> Result {
        let Self { potentialRevert } = self;
        assume_no_revert::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(
            ccx.state,
            ccx.ecx.journaled_state.depth,
            vec![AcceptableRevertParameters::from(potentialRevert)],
        )
    }
}

impl_is_pure_true!(assumeNoRevert_2Call);
impl Cheatcode for assumeNoRevert_2Call {
    fn apply_stateful<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(&self, ccx: &mut CheatsCtxt<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT, DatabaseT>) -> Result {
        let Self { potentialReverts } = self;
        assume_no_revert::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(
            ccx.state,
            ccx.ecx.journaled_state.depth,
            potentialReverts.iter().map(AcceptableRevertParameters::from).collect(),
        )
    }
}

fn assume_no_revert<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
>(
    state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>,
    depth: usize,
    parameters: Vec<AcceptableRevertParameters>,
) -> Result {
    ensure!(
        state.assume_no_revert.is_none(),
        "you must make another external call prior to calling assumeNoRevert again"
    );

    state.assume_no_revert = Some(AssumeNoRevert { depth, reasons: parameters, reverted_by: None });

    Ok(Vec::default())
}
