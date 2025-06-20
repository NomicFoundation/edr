use std::{cmp::Ordering, collections::HashMap};

use alloy_primitives::{Address, Bytes, U256};
use foundry_evm_core::evm_context::{
    BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
    TransactionErrorTrait,
};
use revm::{
    bytecode::Bytecode,
    context::{result::HaltReasonTr, JournalTr},
    interpreter::InstructionResult,
};

use crate::{
    impl_is_pure_true, Cheatcode, CheatcodeBackend, Cheatcodes, CheatsCtxt, Result,
    Vm::{
        clearMockedCallsCall, mockCallRevert_0Call, mockCallRevert_1Call, mockCall_0Call,
        mockCall_1Call,
    },
};

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
        self.calldata
            .cmp(&other.calldata)
            .reverse()
            .then(self.value.cmp(&other.value).reverse())
    }
}

impl_is_pure_true!(clearMockedCallsCall);
impl Cheatcode for clearMockedCallsCall {
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
        state.mocked_calls = HashMap::default();
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCall_0Call);
impl Cheatcode for mockCall_0Call {
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
            callee,
            data,
            returnData,
        } = self;
        // TODO: use ecx.load_account
        let acc = ccx.ecx.journaled_state.load_account(*callee)?;

        // Etches a single byte onto the account if it is empty to circumvent the
        // `extcodesize` check Solidity might perform.
        let empty_bytecode = acc.info.code.as_ref().is_none_or(Bytecode::is_empty);
        if empty_bytecode {
            let code = Bytecode::new_raw(Bytes::from_static(&[0u8]));
            ccx.ecx.journaled_state.set_code(*callee, code);
        }

        mock_call(
            ccx.state,
            callee,
            data,
            None,
            returnData,
            InstructionResult::Return,
        );
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCall_1Call);
impl Cheatcode for mockCall_1Call {
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
            callee,
            msgValue,
            data,
            returnData,
        } = self;
        ccx.ecx.journaled_state.load_account(*callee)?;
        mock_call(
            ccx.state,
            callee,
            data,
            Some(msgValue),
            returnData,
            InstructionResult::Return,
        );
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCallRevert_0Call);
impl Cheatcode for mockCallRevert_0Call {
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
            revertData,
        } = self;
        mock_call(
            state,
            callee,
            data,
            None,
            revertData,
            InstructionResult::Revert,
        );
        Ok(Vec::default())
    }
}

impl_is_pure_true!(mockCallRevert_1Call);
impl Cheatcode for mockCallRevert_1Call {
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
            revertData,
        } = self;
        mock_call(
            state,
            callee,
            data,
            Some(msgValue),
            revertData,
            InstructionResult::Revert,
        );
        Ok(Vec::default())
    }
}

#[allow(clippy::ptr_arg)] // Not public API, doesn't matter
fn mock_call<
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
    callee: &Address,
    cdata: &Bytes,
    value: Option<&U256>,
    rdata: &Bytes,
    ret_type: InstructionResult,
) {
    state.mocked_calls.entry(*callee).or_default().insert(
        MockCallDataContext {
            calldata: Bytes::copy_from_slice(cdata),
            value: value.copied(),
        },
        MockCallReturnData {
            ret_type,
            data: Bytes::copy_from_slice(rdata),
        },
    );
}
