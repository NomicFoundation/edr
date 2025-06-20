use alloy_primitives::Address;
use foundry_evm_core::evm_context::{
    BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
    TransactionErrorTrait,
};
use revm::{context::result::HaltReasonTr, context_interface::JournalTr};

use crate::{
    impl_is_pure_true, Cheatcode, CheatcodeBackend, Cheatcodes, CheatsCtxt, Result,
    Vm::{prank_0Call, prank_1Call, startPrank_0Call, startPrank_1Call, stopPrankCall},
};

/// Prank information.
#[derive(Clone, Debug, Default)]
pub struct Prank {
    /// Address of the contract that initiated the prank
    pub prank_caller: Address,
    /// Address of `tx.origin` when the prank was initiated
    pub prank_origin: Address,
    /// The address to assign to `msg.sender`
    pub new_caller: Address,
    /// The address to assign to `tx.origin`
    pub new_origin: Option<Address>,
    /// The depth at which the prank was called
    pub depth: u64,
    /// Whether the prank stops by itself after the next call
    pub single_call: bool,
    /// Whether the prank has been used yet (false if unused)
    pub used: bool,
}

impl Prank {
    /// Create a new prank.
    pub fn new(
        prank_caller: Address,
        prank_origin: Address,
        new_caller: Address,
        new_origin: Option<Address>,
        depth: u64,
        single_call: bool,
    ) -> Prank {
        Prank {
            prank_caller,
            prank_origin,
            new_caller,
            new_origin,
            depth,
            single_call,
            used: false,
        }
    }

    /// Apply the prank by setting `used` to true iff it is false
    /// Only returns self in the case it is updated (first application)
    pub fn first_time_applied(&self) -> Option<Self> {
        if self.used {
            None
        } else {
            Some(Prank {
                used: true,
                ..self.clone()
            })
        }
    }
}

impl_is_pure_true!(prank_0Call);
impl Cheatcode for prank_0Call {
    fn apply_full<
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
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { msgSender } = self;
        prank(ccx, msgSender, None, true)
    }
}

impl_is_pure_true!(startPrank_0Call);
impl Cheatcode for startPrank_0Call {
    fn apply_full<
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
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { msgSender } = self;
        prank(ccx, msgSender, None, false)
    }
}

impl_is_pure_true!(prank_1Call);
impl Cheatcode for prank_1Call {
    fn apply_full<
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
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            msgSender,
            txOrigin,
        } = self;
        prank(ccx, msgSender, Some(txOrigin), true)
    }
}

impl_is_pure_true!(startPrank_1Call);
impl Cheatcode for startPrank_1Call {
    fn apply_full<
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
    >(
        &self,
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            msgSender,
            txOrigin,
        } = self;
        prank(ccx, msgSender, Some(txOrigin), false)
    }
}

impl_is_pure_true!(stopPrankCall);
impl Cheatcode for stopPrankCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self {} = self;
        state.prank = None;
        Ok(Vec::default())
    }
}

fn prank<
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
>(
    ccx: &mut CheatsCtxt<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    >,
    new_caller: &Address,
    new_origin: Option<&Address>,
    single_call: bool,
) -> Result {
    let prank = Prank::new(
        ccx.caller,
        ccx.ecx.tx.caller(),
        *new_caller,
        new_origin.copied(),
        ccx.ecx.journaled_state.depth() as u64,
        single_call,
    );

    if let Some(Prank {
        used,
        single_call: current_single_call,
        ..
    }) = ccx.state.prank
    {
        ensure!(
            used,
            "cannot overwrite a prank until it is applied at least once"
        );
        // This case can only fail if the user calls `vm.startPrank` and then `vm.prank`
        // later on. This should not be possible without first calling
        // `stopPrank`
        ensure!(
            single_call == current_single_call,
            "cannot override an ongoing prank with a single vm.prank; \
             use vm.startPrank to override the current prank"
        );
    }

    ccx.state.prank = Some(prank);
    Ok(Vec::default())
}
