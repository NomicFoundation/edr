use std::collections::{hash_map::Entry, HashMap};

use alloy_primitives::{address, Address, Bytes, LogData as RawLog, U256};
use alloy_sol_types::{SolError, SolValue};
use foundry_evm_core::evm_context::{
    BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
    TransactionErrorTrait,
};
use revm::{
    context::result::HaltReasonTr,
    context_interface::JournalTr,
    interpreter::{return_ok, InstructionResult},
};
use spec::Vm;

use crate::{
    impl_is_pure_true, Cheatcode, CheatcodeBackend, Cheatcodes, CheatsCtxt, Result,
    Vm::{
        _expectCheatcodeRevert_0Call, _expectCheatcodeRevert_1Call, _expectCheatcodeRevert_2Call,
        expectCallMinGas_0Call, expectCallMinGas_1Call, expectCall_0Call, expectCall_1Call,
        expectCall_2Call, expectCall_3Call, expectCall_4Call, expectCall_5Call, expectEmit_0Call,
        expectEmit_1Call, expectEmit_2Call, expectEmit_3Call, expectRevert_0Call,
        expectRevert_1Call, expectRevert_2Call, expectSafeMemoryCall, expectSafeMemoryCallCall,
        stopExpectSafeMemoryCall,
    },
};

/// For some cheatcodes we may internally change the status of the call, i.e. in
/// `expectRevert`. Solidity will see a successful call and attempt to decode
/// the return data. Therefore, we need to populate the return with dummy bytes
/// so the decode doesn't fail.
///
/// 8192 bytes was arbitrarily chosen because it is long enough for return
/// values up to 256 words in size.
static DUMMY_CALL_OUTPUT: Bytes = Bytes::from_static(&[0u8; 8192]);

/// Same reasoning as [`DUMMY_CALL_OUTPUT`], but for creates.
const DUMMY_CREATE_ADDRESS: Address = address!("0000000000000000000000000000000000000001");

/// Tracks the expected calls per address.
///
/// For each address, we track the expected calls per call data. We track it in
/// such manner so that we don't mix together calldatas that only contain
/// selectors and calldatas that contain selector and arguments (partial and
/// full matches).
///
/// This then allows us to customize the matching behavior for each call data on
/// the `ExpectedCallData` struct and track how many times we've actually seen
/// the call on the second element of the tuple.
pub type ExpectedCallTracker = HashMap<Address, HashMap<Bytes, (ExpectedCallData, u64)>>;

#[derive(Clone, Debug)]
pub struct ExpectedCallData {
    /// The expected value sent in the call
    pub value: Option<U256>,
    /// The expected gas supplied to the call
    pub gas: Option<u64>,
    /// The expected *minimum* gas supplied to the call
    pub min_gas: Option<u64>,
    /// The number of times the call is expected to be made.
    /// If the type of call is `NonCount`, this is the lower bound for the
    /// number of calls that must be seen.
    /// If the type of call is `Count`, this is the exact number of calls that
    /// must be seen.
    pub count: u64,
    /// The type of expected call.
    pub call_type: ExpectedCallType,
}

/// The type of expected call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpectedCallType {
    /// The call is expected to be made at least once.
    NonCount,
    /// The exact number of calls expected.
    Count,
}

/// The type of expected revert.
#[derive(Clone, Debug)]
pub enum ExpectedRevertKind {
    /// Expects revert from the next non-cheatcode call.
    Default,
    /// Expects revert from the next cheatcode call.
    ///
    /// The `pending_processing` flag is used to track whether we have exited
    /// `expectCheatcodeRevert` context or not.
    /// We have to track it to avoid expecting `expectCheatcodeRevert` call to
    /// revert itself.
    Cheatcode { pending_processing: bool },
}

#[derive(Clone, Debug)]
pub struct ExpectedRevert {
    /// The expected data returned by the revert, None being any
    pub reason: Option<Vec<u8>>,
    /// The depth at which the revert is expected
    pub depth: u64,
    /// The type of expected revert.
    pub kind: ExpectedRevertKind,
}

#[derive(Clone, Debug)]
pub struct ExpectedEmit {
    /// The depth at which we expect this emit to have occurred
    pub depth: u64,
    /// The log we expect
    pub log: Option<RawLog>,
    /// The checks to perform:
    /// ```text
    /// ┌───────┬───────┬───────┬────┐
    /// │topic 1│topic 2│topic 3│data│
    /// └───────┴───────┴───────┴────┘
    /// ```
    pub checks: [bool; 4],
    /// If present, check originating address against this
    pub address: Option<Address>,
    /// Whether the log was actually found in the subcalls
    pub found: bool,
}

impl_is_pure_true!(expectCall_0Call);
impl Cheatcode for expectCall_0Call {
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
        let Self { callee, data } = self;
        expect_call(
            state,
            callee,
            data,
            None,
            None,
            None,
            1,
            ExpectedCallType::NonCount,
        )
    }
}

impl_is_pure_true!(expectCall_1Call);
impl Cheatcode for expectCall_1Call {
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
        let Self {
            callee,
            data,
            count,
        } = self;
        expect_call(
            state,
            callee,
            data,
            None,
            None,
            None,
            *count,
            ExpectedCallType::Count,
        )
    }
}

impl_is_pure_true!(expectCall_2Call);
impl Cheatcode for expectCall_2Call {
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
        let Self {
            callee,
            msgValue,
            data,
        } = self;
        expect_call(
            state,
            callee,
            data,
            Some(msgValue),
            None,
            None,
            1,
            ExpectedCallType::NonCount,
        )
    }
}

impl_is_pure_true!(expectCall_3Call);
impl Cheatcode for expectCall_3Call {
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
        let Self {
            callee,
            msgValue,
            data,
            count,
        } = self;
        expect_call(
            state,
            callee,
            data,
            Some(msgValue),
            None,
            None,
            *count,
            ExpectedCallType::Count,
        )
    }
}

impl_is_pure_true!(expectCall_4Call);
impl Cheatcode for expectCall_4Call {
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
        let Self {
            callee,
            msgValue,
            gas,
            data,
        } = self;
        expect_call(
            state,
            callee,
            data,
            Some(msgValue),
            Some(*gas),
            None,
            1,
            ExpectedCallType::NonCount,
        )
    }
}

impl_is_pure_true!(expectCall_5Call);
impl Cheatcode for expectCall_5Call {
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
        let Self {
            callee,
            msgValue,
            gas,
            data,
            count,
        } = self;
        expect_call(
            state,
            callee,
            data,
            Some(msgValue),
            Some(*gas),
            None,
            *count,
            ExpectedCallType::Count,
        )
    }
}

impl_is_pure_true!(expectCallMinGas_0Call);
impl Cheatcode for expectCallMinGas_0Call {
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
        let Self {
            callee,
            msgValue,
            minGas,
            data,
        } = self;
        expect_call(
            state,
            callee,
            data,
            Some(msgValue),
            None,
            Some(*minGas),
            1,
            ExpectedCallType::NonCount,
        )
    }
}

impl_is_pure_true!(expectCallMinGas_1Call);
impl Cheatcode for expectCallMinGas_1Call {
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
        let Self {
            callee,
            msgValue,
            minGas,
            data,
            count,
        } = self;
        expect_call(
            state,
            callee,
            data,
            Some(msgValue),
            None,
            Some(*minGas),
            *count,
            ExpectedCallType::Count,
        )
    }
}

impl_is_pure_true!(expectEmit_0Call);
impl Cheatcode for expectEmit_0Call {
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
            checkTopic1,
            checkTopic2,
            checkTopic3,
            checkData,
        } = *self;
        expect_emit(
            ccx.state,
            ccx.ecx.journaled_state.depth() as u64,
            [checkTopic1, checkTopic2, checkTopic3, checkData],
            None,
        )
    }
}

impl_is_pure_true!(expectEmit_1Call);
impl Cheatcode for expectEmit_1Call {
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
            checkTopic1,
            checkTopic2,
            checkTopic3,
            checkData,
            emitter,
        } = *self;
        expect_emit(
            ccx.state,
            ccx.ecx.journaled_state.depth() as u64,
            [checkTopic1, checkTopic2, checkTopic3, checkData],
            Some(emitter),
        )
    }
}

impl_is_pure_true!(expectEmit_2Call);
impl Cheatcode for expectEmit_2Call {
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
        let Self {} = self;
        expect_emit(
            ccx.state,
            ccx.ecx.journaled_state.depth() as u64,
            [true; 4],
            None,
        )
    }
}

impl_is_pure_true!(expectEmit_3Call);
impl Cheatcode for expectEmit_3Call {
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
        let Self { emitter } = *self;
        expect_emit(
            ccx.state,
            ccx.ecx.journaled_state.depth() as u64,
            [true; 4],
            Some(emitter),
        )
    }
}

impl_is_pure_true!(expectRevert_0Call);
impl Cheatcode for expectRevert_0Call {
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
        let Self {} = self;
        expect_revert(
            ccx.state,
            None,
            ccx.ecx.journaled_state.depth() as u64,
            false,
        )
    }
}

impl_is_pure_true!(expectRevert_1Call);
impl Cheatcode for expectRevert_1Call {
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
        let Self { revertData } = self;
        expect_revert(
            ccx.state,
            Some(revertData.as_ref()),
            ccx.ecx.journaled_state.depth() as u64,
            false,
        )
    }
}

impl_is_pure_true!(expectRevert_2Call);
impl Cheatcode for expectRevert_2Call {
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
        let Self { revertData } = self;
        expect_revert(
            ccx.state,
            Some(revertData),
            ccx.ecx.journaled_state.depth() as u64,
            false,
        )
    }
}

impl_is_pure_true!(_expectCheatcodeRevert_0Call);
impl Cheatcode for _expectCheatcodeRevert_0Call {
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
        expect_revert(
            ccx.state,
            None,
            ccx.ecx.journaled_state.depth() as u64,
            true,
        )
    }
}

impl_is_pure_true!(_expectCheatcodeRevert_1Call);
impl Cheatcode for _expectCheatcodeRevert_1Call {
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
        let Self { revertData } = self;
        expect_revert(
            ccx.state,
            Some(revertData.as_ref()),
            ccx.ecx.journaled_state.depth() as u64,
            true,
        )
    }
}

impl_is_pure_true!(_expectCheatcodeRevert_2Call);
impl Cheatcode for _expectCheatcodeRevert_2Call {
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
        let Self { revertData } = self;
        expect_revert(
            ccx.state,
            Some(revertData),
            ccx.ecx.journaled_state.depth() as u64,
            true,
        )
    }
}

impl_is_pure_true!(expectSafeMemoryCall);
impl Cheatcode for expectSafeMemoryCall {
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
        let Self { min, max } = *self;
        expect_safe_memory(ccx.state, min, max, ccx.ecx.journaled_state.depth() as u64)
    }
}

impl_is_pure_true!(stopExpectSafeMemoryCall);
impl Cheatcode for stopExpectSafeMemoryCall {
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
        let Self {} = self;
        ccx.state
            .allowed_mem_writes
            .remove(&(ccx.ecx.journaled_state.depth() as u64));
        Ok(Vec::default())
    }
}

impl_is_pure_true!(expectSafeMemoryCallCall);
impl Cheatcode for expectSafeMemoryCallCall {
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
        let Self { min, max } = *self;
        expect_safe_memory(
            ccx.state,
            min,
            max,
            ccx.ecx.journaled_state.depth() as u64 + 1,
        )
    }
}

///  Handles expected calls specified by the `expectCall` cheatcodes.
///
///  It can handle calls in two ways:
///  - If the cheatcode was used with a `count` argument, it will expect the
///    call to be made exactly `count` times. e.g.
///    `vm.expectCall(address(0xc4f3), abi.encodeWithSelector(0xd34db33f), 4)`
///    will expect the call to address(0xc4f3) with selector `0xd34db33f` to be
///    made exactly 4 times. If the amount of calls is less or more than 4, the
///    test will fail. Note that the `count` argument cannot be overwritten with
///    another `vm.expectCall`. If this is attempted, `expectCall` will revert.
///  - If the cheatcode was used without a `count` argument, it will expect the
///    call to be made at least the amount of times the cheatcode was called.
///    This means that `vm.expectCall` without a count argument can be called
///    many times, but cannot be called with a `count` argument after it was
///    called without one. If the latter happens, `expectCall` will revert. e.g
///    `vm.expectCall(address(0xc4f3), abi.encodeWithSelector(0xd34db33f))` will
///    expect the call to address(0xc4f3) and selector `0xd34db33f` to be made
///    at  least once. If the amount of calls is 0, the test will fail. If the
///    call is made more than once, the test will pass.
#[allow(clippy::too_many_arguments)] // It is what it is
fn expect_call<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
>(
    state: &mut Cheatcodes<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    target: &Address,
    calldata: &Bytes,
    value: Option<&U256>,
    mut gas: Option<u64>,
    mut min_gas: Option<u64>,
    count: u64,
    call_type: ExpectedCallType,
) -> Result {
    // False positive
    #[allow(clippy::mutable_key_type)]
    let expecteds = state.expected_calls.entry(*target).or_default();

    if let Some(val) = value {
        if *val > U256::ZERO {
            // If the value of the transaction is non-zero, the EVM adds a call stipend of
            // 2300 gas to ensure that the basic fallback function can be
            // called.
            let positive_value_cost_stipend = 2300;
            if let Some(gas) = &mut gas {
                *gas += positive_value_cost_stipend;
            }
            if let Some(min_gas) = &mut min_gas {
                *min_gas += positive_value_cost_stipend;
            }
        }
    }

    match call_type {
        ExpectedCallType::Count => {
            // Get the expected calls for this target.
            // In this case, as we're using counted expectCalls, we should not be able to
            // set them more than once.
            ensure!(
                !expecteds.contains_key(calldata),
                "counted expected calls can only bet set once"
            );
            expecteds.insert(
                calldata.clone(),
                (
                    ExpectedCallData {
                        value: value.copied(),
                        gas,
                        min_gas,
                        count,
                        call_type,
                    },
                    0,
                ),
            );
        }
        ExpectedCallType::NonCount => {
            // Check if the expected calldata exists.
            // If it does, increment the count by one as we expect to see it one more time.
            match expecteds.entry(calldata.clone()) {
                Entry::Occupied(mut entry) => {
                    let (expected, _) = entry.get_mut();
                    // Ensure we're not overwriting a counted expectCall.
                    ensure!(
                        expected.call_type == ExpectedCallType::NonCount,
                        "cannot overwrite a counted expectCall with a non-counted expectCall"
                    );
                    expected.count += 1;
                }
                // If it does not exist, then create it.
                Entry::Vacant(entry) => {
                    entry.insert((
                        ExpectedCallData {
                            value: value.copied(),
                            gas,
                            min_gas,
                            count,
                            call_type,
                        },
                        0,
                    ));
                }
            }
        }
    }

    Ok(Vec::default())
}

fn expect_emit<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
>(
    state: &mut Cheatcodes<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    depth: u64,
    checks: [bool; 4],
    address: Option<Address>,
) -> Result {
    state.expected_emits.push_back(ExpectedEmit {
        depth,
        checks,
        address,
        found: false,
        log: None,
    });
    Ok(Vec::default())
}

pub(crate) fn handle_expect_emit<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
>(
    state: &mut Cheatcodes<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    log: &alloy_primitives::Log,
) {
    // Fill or check the expected emits.
    // We expect for emit checks to be filled as they're declared (from oldest to
    // newest), so we fill them and push them to the back of the queue.
    // If the user has properly filled all the emits, they'll end up in their
    // original order. If not, the queue will not be in the order the events
    // will be intended to be filled, and we'll be able to later detect this and
    // bail.

    // First, we can return early if all events have been matched.
    // This allows a contract to arbitrarily emit more events than expected
    // (additive behavior), as long as all the previous events were matched in
    // the order they were expected to be.
    if state.expected_emits.iter().all(|expected| expected.found) {
        return;
    }

    // if there's anything to fill, we need to pop back.
    // Otherwise, if there are any events that are unmatched, we try to match to
    // match them in the order declared, so we start popping from the front
    // (like a queue).
    let mut event_to_fill_or_check = if state
        .expected_emits
        .iter()
        .any(|expected| expected.log.is_none())
    {
        state.expected_emits.pop_back()
    } else {
        state.expected_emits.pop_front()
    }
    .expect("we should have an emit to fill or check");

    let Some(expected) = &event_to_fill_or_check.log else {
        // Fill the event.
        event_to_fill_or_check.log = Some(log.data.clone());
        state.expected_emits.push_back(event_to_fill_or_check);
        return;
    };

    let expected_topic_0 = expected.topics().first();
    let log_topic_0 = log.topics().first();

    if expected_topic_0
        .zip(log_topic_0)
        .is_some_and(|(a, b)| a == b && expected.topics().len() == log.topics().len())
    {
        // Match topics
        event_to_fill_or_check.found = log
            .topics()
            .iter()
            .skip(1)
            .enumerate()
            .filter(|(i, _)| event_to_fill_or_check.checks[*i])
            .all(|(i, topic)| topic == &expected.topics()[i + 1]);

        // Maybe match source address
        if let Some(addr) = event_to_fill_or_check.address {
            event_to_fill_or_check.found &= addr == log.address;
        }

        // Maybe match data
        if event_to_fill_or_check.checks[3] {
            event_to_fill_or_check.found &= expected.data.as_ref() == log.data.data.as_ref();
        }
    }

    // If we found the event, we can push it to the back of the queue
    // and begin expecting the next event.
    if event_to_fill_or_check.found {
        state.expected_emits.push_back(event_to_fill_or_check);
    } else {
        // We did not match this event, so we need to keep waiting for the right one to
        // appear.
        state.expected_emits.push_front(event_to_fill_or_check);
    }
}

fn expect_revert<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
>(
    state: &mut Cheatcodes<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    reason: Option<&[u8]>,
    depth: u64,
    cheatcode: bool,
) -> Result {
    ensure!(
        state.expected_revert.is_none(),
        "you must call another function prior to expecting a second revert"
    );
    state.expected_revert = Some(ExpectedRevert {
        reason: reason.map(<[_]>::to_vec),
        depth,
        kind: if cheatcode {
            ExpectedRevertKind::Cheatcode {
                pending_processing: true,
            }
        } else {
            ExpectedRevertKind::Default
        },
    });
    Ok(Vec::default())
}

pub(crate) fn handle_expect_revert(
    is_create: bool,
    expected_revert: Option<&[u8]>,
    status: InstructionResult,
    retdata: Bytes,
) -> Result<(Option<Address>, Bytes)> {
    let success_return = || {
        if is_create {
            (Some(DUMMY_CREATE_ADDRESS), Bytes::new())
        } else {
            (None, DUMMY_CALL_OUTPUT.clone())
        }
    };

    ensure!(
        !matches!(status, return_ok!()),
        "call did not revert as expected"
    );

    // If None, accept any revert
    let Some(expected_revert) = expected_revert else {
        return Ok(success_return());
    };

    if !expected_revert.is_empty() && retdata.is_empty() {
        bail!("call reverted as expected, but without data");
    }

    let mut actual_revert: Vec<u8> = retdata.into();

    // Try decoding as known errors
    if matches!(
        actual_revert.get(..4).map(|s| s.try_into().unwrap()),
        Some(Vm::CheatcodeError::SELECTOR | alloy_sol_types::Revert::SELECTOR)
    ) {
        if let Ok(decoded) = Vec::<u8>::abi_decode(&actual_revert[4..], false) {
            actual_revert = decoded;
        }
    }

    if actual_revert == expected_revert {
        Ok(success_return())
    } else {
        let stringify = |data: &[u8]| {
            String::abi_decode(data, false)
                .ok()
                .or_else(|| std::str::from_utf8(data).ok().map(ToOwned::to_owned))
                .unwrap_or_else(|| hex::encode_prefixed(data))
        };
        Err(fmt_err!(
            "Error != expected error: {} != {}",
            stringify(&actual_revert),
            stringify(expected_revert),
        ))
    }
}

fn expect_safe_memory<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
>(
    state: &mut Cheatcodes<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    start: u64,
    end: u64,
    depth: u64,
) -> Result {
    ensure!(
        start < end,
        "memory range start ({start}) is greater than end ({end})"
    );
    #[allow(clippy::single_range_in_vec_init)] // Wanted behaviour
    let offsets = state
        .allowed_mem_writes
        .entry(depth)
        .or_insert_with(|| vec![0..0x60]);
    offsets.push(start..end);
    Ok(Vec::default())
}
