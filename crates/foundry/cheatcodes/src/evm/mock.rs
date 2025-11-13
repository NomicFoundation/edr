#[allow(clippy::wildcard_imports)]
use crate::{impl_is_pure_true, Cheatcode, Cheatcodes, CheatsCtxt, Result, Vm::*};
use alloy_primitives::{Address, Bytes, U256, map::HashMap};
use foundry_evm_core::{
    backend::CheatcodeBackend,
    evm_context::{BlockEnvTr, ChainContextTr, EvmBuilderTrait, TransactionEnvTr, TransactionErrorTrait},
};
use revm::{bytecode::Bytecode, context::JournalTr, interpreter::InstructionResult};
use std::{cmp::Ordering, collections::VecDeque};
use revm::context::result::HaltReasonTr;
use foundry_evm_core::evm_context::HardforkTr;

/// Mocked call data.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MockCallDataContext {
    /// The partial calldata to match for mock
    pub calldata: Bytes,
    /// The value to match for mock
    pub value: Option<U256>,
}

/// Mocked return data.
#[derive(Clone, Debug)]
pub struct MockCallReturnData {
    /// The return type for the mocked call
    pub ret_type: InstructionResult,
    /// Return data or error
    pub data: Bytes,
}

impl PartialOrd for MockCallDataContext {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MockCallDataContext {
    fn cmp(&self, other: &Self) -> Ordering {
        // Calldata matching is reversed to ensure that a tighter match is
        // returned if an exact match is not found. In case, there is
        // a partial match to calldata that is more specific than
        // a match to a msg.value, then the more specific calldata takes
        // precedence.
        self.calldata.cmp(&other.calldata).reverse().then(self.value.cmp(&other.value).reverse())
    }
}

impl_is_pure_true!(clearMockedCallsCall);
impl Cheatcode for clearMockedCallsCall {
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
    >(&self, state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>) -> Result {
        let Self {} = self;
        state.mocked_calls = HashMap::default();
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCall_0Call);
impl Cheatcode for mockCall_0Call {
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
        let Self { callee, data, returnData } = self;
        let _ = make_acc_non_empty(callee, ccx)?;

        mock_call::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(ccx.state, callee, data, None, returnData, InstructionResult::Return);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCall_1Call);
impl Cheatcode for mockCall_1Call {
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
        let Self { callee, msgValue, data, returnData } = self;
        ccx.ecx.journaled_state.load_account(*callee)?;
        mock_call::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(ccx.state, callee, data, Some(msgValue), returnData, InstructionResult::Return);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCall_2Call);
impl Cheatcode for mockCall_2Call {
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
        let Self { callee, data, returnData } = self;
        let _ = make_acc_non_empty(callee, ccx)?;

        mock_call::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(
            ccx.state,
            callee,
            &Bytes::from(*data),
            None,
            returnData,
            InstructionResult::Return,
        );
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCall_3Call);
impl Cheatcode for mockCall_3Call {
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
        let Self { callee, msgValue, data, returnData } = self;
        ccx.ecx.journaled_state.load_account(*callee)?;
        mock_call::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(
            ccx.state,
            callee,
            &Bytes::from(*data),
            Some(msgValue),
            returnData,
            InstructionResult::Return,
        );
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCalls_0Call);
impl Cheatcode for mockCalls_0Call {
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
        let Self { callee, data, returnData } = self;
        let _ = make_acc_non_empty(callee, ccx)?;

        mock_calls::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(ccx.state, callee, data, None, returnData, InstructionResult::Return);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCalls_1Call);
impl Cheatcode for mockCalls_1Call {
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
        let Self { callee, msgValue, data, returnData } = self;
        ccx.ecx.journaled_state.load_account(*callee)?;
        mock_calls::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(ccx.state, callee, data, Some(msgValue), returnData, InstructionResult::Return);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCallRevert_0Call);
impl Cheatcode for mockCallRevert_0Call {
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
        let Self { callee, data, revertData } = self;
        let _ = make_acc_non_empty(callee, ccx)?;

        mock_call::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(ccx.state, callee, data, None, revertData, InstructionResult::Revert);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCallRevert_1Call);
impl Cheatcode for mockCallRevert_1Call {
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
        let Self { callee, msgValue, data, revertData } = self;
        let _ = make_acc_non_empty(callee, ccx)?;

        mock_call::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(ccx.state, callee, data, Some(msgValue), revertData, InstructionResult::Revert);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCallRevert_2Call);
impl Cheatcode for mockCallRevert_2Call {
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
        let Self { callee, data, revertData } = self;
        let _ = make_acc_non_empty(callee, ccx)?;

        mock_call::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(
            ccx.state,
            callee,
            &Bytes::from(*data),
            None,
            revertData,
            InstructionResult::Revert,
        );
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCallRevert_3Call);
impl Cheatcode for mockCallRevert_3Call {
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
        let Self { callee, msgValue, data, revertData } = self;
        let _ = make_acc_non_empty(callee, ccx)?;

        mock_call::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(
            ccx.state,
            callee,
            &Bytes::from(*data),
            Some(msgValue),
            revertData,
            InstructionResult::Revert,
        );
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockFunctionCall);
impl Cheatcode for mockFunctionCall {
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
    >(&self, state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>) -> Result {
        let Self { callee, target, data } = self;
        state.mocked_functions.entry(*callee).or_default().insert(data.clone(), *target);

        Ok(Vec::default())
    }
}

fn mock_call<

    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
>(
    state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>,
    callee: &Address,
    cdata: &Bytes,
    value: Option<&U256>,
    rdata: &Bytes,
    ret_type: InstructionResult,
) {
    mock_calls::<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>(state, callee, cdata, value, std::slice::from_ref(rdata), ret_type);
}

fn mock_calls<

    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
>(
    state: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>,
    callee: &Address,
    cdata: &Bytes,
    value: Option<&U256>,
    rdata_vec: &[Bytes],
    ret_type: InstructionResult,
) {
    state.mocked_calls.entry(*callee).or_default().insert(
        MockCallDataContext { calldata: Bytes::copy_from_slice(cdata), value: value.copied() },
        rdata_vec
            .iter()
            .map(|rdata| MockCallReturnData { ret_type, data: rdata.clone() })
            .collect::<VecDeque<_>>(),
    );
}

// Etches a single byte onto the account if it is empty to circumvent the `extcodesize`
// check Solidity might perform.
fn make_acc_non_empty<
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
>(callee: &Address, ccx: &mut CheatsCtxt<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT, DatabaseT>) -> Result {
    let acc = ccx.ecx.journaled_state.load_account(*callee)?;

    let empty_bytecode = acc.info.code.as_ref().is_none_or(Bytecode::is_empty);
    if empty_bytecode {
        let code = Bytecode::new_raw(Bytes::from_static(&[0u8]));
        ccx.ecx.journaled_state.set_code(*callee, code);
    }

    Ok(Vec::default())
}
