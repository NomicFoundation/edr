//! Implementations of [`Utilities`](spec::Group::Utilities) cheatcodes.

use alloy_dyn_abi::{DynSolType, DynSolValue, Resolver, TypedData};
use alloy_ens::namehash;
use alloy_primitives::{aliases::B32, keccak256, map::HashMap, Bytes, B64, U256};
use alloy_sol_types::SolValue;
use edr_solidity_collector_eip712::{collector::Eip712TypeCollection, Eip712Type};
use foundry_evm_core::{
    backend::CheatcodeBackend,
    constants::DEFAULT_CREATE2_DEPLOYER,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use proptest::prelude::Strategy;
use rand::{seq::SliceRandom, Rng, RngCore};
use revm::{context::result::HaltReasonTr, context_interface::JournalTr as _};

#[allow(clippy::wildcard_imports)]
use crate::{
    impl_is_pure_false, impl_is_pure_true, Cheatcode, Cheatcodes, CheatcodesExecutor, CheatsCtxt,
    Result, Vm::*,
};

/// Contains locations of traces ignored via cheatcodes.
///
/// The way we identify location in traces is by `(node_idx, item_idx)` tuple
/// where `node_idx` is an index of a call trace node, and `item_idx` is a value
/// between 0 and `node.ordering.len()` where i represents point after ith item,
/// and 0 represents the beginning of the node trace.
#[derive(Debug, Default, Clone)]
pub struct IgnoredTraces {
    /// Mapping from `(start_node_idx, start_item_idx)` to `(end_node_idx,
    /// end_item_idx)` representing ranges of trace nodes to ignore.
    pub ignored: HashMap<(usize, usize), (usize, usize)>,
    /// Keeps track of `(start_node_idx, start_item_idx)` of the last
    /// `vm.pauseTracing` call.
    pub last_pause_call: Option<(usize, usize)>,
}

impl_is_pure_true!(labelCall);
impl Cheatcode for labelCall {
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
        let Self { account, newLabel } = self;
        state.labels.insert(*account, newLabel.clone());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(getLabelCall);
impl Cheatcode for getLabelCall {
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
        let Self { account } = self;
        Ok(match state.labels.get(account) {
            Some(label) => label.abi_encode(),
            None => format!("unlabeled:{account}").abi_encode(),
        })
    }
}

impl_is_pure_true!(computeCreateAddressCall);
impl Cheatcode for computeCreateAddressCall {
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
    >(
        &self,
        _state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { nonce, deployer } = self;
        ensure!(
            *nonce <= U256::from(u64::MAX),
            "nonce must be less than 2^64 - 1"
        );
        Ok(deployer.create(nonce.to()).abi_encode())
    }
}

impl_is_pure_true!(computeCreate2Address_0Call);
impl Cheatcode for computeCreate2Address_0Call {
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
    >(
        &self,
        _state: &mut Cheatcodes<
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
            salt,
            initCodeHash,
            deployer,
        } = self;
        Ok(deployer.create2(salt, initCodeHash).abi_encode())
    }
}

impl_is_pure_true!(computeCreate2Address_1Call);
impl Cheatcode for computeCreate2Address_1Call {
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
    >(
        &self,
        _state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { salt, initCodeHash } = self;
        Ok(DEFAULT_CREATE2_DEPLOYER
            .create2(salt, initCodeHash)
            .abi_encode())
    }
}

impl_is_pure_true!(ensNamehashCall);
impl Cheatcode for ensNamehashCall {
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
    >(
        &self,
        _state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { name } = self;
        Ok(namehash(name).abi_encode())
    }
}

impl_is_pure_false!(randomUint_0Call);
impl Cheatcode for randomUint_0Call {
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
        random_uint(state, None, None)
    }
}

impl_is_pure_false!(randomUint_1Call);
impl Cheatcode for randomUint_1Call {
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
        let Self { min, max } = *self;
        random_uint(state, None, Some((min, max)))
    }
}

impl_is_pure_false!(randomUint_2Call);
impl Cheatcode for randomUint_2Call {
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
        let Self { bits } = *self;
        random_uint(state, Some(bits), None)
    }
}

impl_is_pure_false!(randomAddressCall);
impl Cheatcode for randomAddressCall {
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
        Ok(DynSolValue::type_strategy(&DynSolType::Address)
            .new_tree(state.test_runner())
            .unwrap()
            .current()
            .abi_encode())
    }
}

impl_is_pure_false!(randomInt_0Call);
impl Cheatcode for randomInt_0Call {
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
        random_int(state, None)
    }
}

impl_is_pure_false!(randomInt_1Call);
impl Cheatcode for randomInt_1Call {
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
        let Self { bits } = *self;
        random_int(state, Some(bits))
    }
}

impl_is_pure_false!(randomBoolCall);
impl Cheatcode for randomBoolCall {
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
        let rand_bool: bool = state.rng().random();
        Ok(rand_bool.abi_encode())
    }
}

impl_is_pure_false!(randomBytesCall);
impl Cheatcode for randomBytesCall {
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
        let Self { len } = *self;
        ensure!(
            len <= U256::from(usize::MAX),
            format!("bytes length cannot exceed {}", usize::MAX)
        );
        let mut bytes = vec![0u8; len.to::<usize>()];
        state.rng().fill_bytes(&mut bytes);
        Ok(bytes.abi_encode())
    }
}

impl_is_pure_false!(randomBytes4Call);
impl Cheatcode for randomBytes4Call {
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
        let rand_u32 = state.rng().next_u32();
        Ok(B32::from(rand_u32).abi_encode())
    }
}

impl_is_pure_false!(randomBytes8Call);
impl Cheatcode for randomBytes8Call {
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
        let rand_u64 = state.rng().next_u64();
        Ok(B64::from(rand_u64).abi_encode())
    }
}

impl_is_pure_true!(pauseTracingCall);
impl Cheatcode for pauseTracingCall {
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
            '_,
            '_,
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
        executor: &mut dyn CheatcodesExecutor<
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
        let Some(tracer) = executor.tracing_inspector().and_then(|t| t.as_ref()) else {
            // No tracer -> nothing to pause
            return Ok(Vec::default());
        };

        // If paused earlier, ignore the call
        if ccx.state.ignored_traces.last_pause_call.is_some() {
            return Ok(Vec::default());
        }

        let cur_node = &tracer.traces().nodes().last().expect("no trace nodes");
        ccx.state.ignored_traces.last_pause_call = Some((cur_node.idx, cur_node.ordering.len()));

        Ok(Vec::default())
    }
}

impl_is_pure_true!(resumeTracingCall);
impl Cheatcode for resumeTracingCall {
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
            '_,
            '_,
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
        executor: &mut dyn CheatcodesExecutor<
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
        let Some(tracer) = executor.tracing_inspector().and_then(|t| t.as_ref()) else {
            // No tracer -> nothing to unpause
            return Ok(Vec::default());
        };

        let Some(start) = ccx.state.ignored_traces.last_pause_call.take() else {
            // Nothing to unpause
            return Ok(Vec::default());
        };

        let node = &tracer.traces().nodes().last().expect("no trace nodes");
        ccx.state
            .ignored_traces
            .ignored
            .insert(start, (node.idx, node.ordering.len()));

        Ok(Vec::default())
    }
}

impl_is_pure_true!(interceptInitcodeCall);
impl Cheatcode for interceptInitcodeCall {
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
        if !state.intercept_next_create_call {
            state.intercept_next_create_call = true;
        } else {
            bail!("vm.interceptInitcode() has already been called")
        }
        Ok(Vec::default())
    }
}

impl_is_pure_true!(setArbitraryStorage_0Call);
impl Cheatcode for setArbitraryStorage_0Call {
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
    >(
        &self,
        ccx: &mut CheatsCtxt<
            '_,
            '_,
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
        let Self { target } = self;
        ccx.state.arbitrary_storage().mark_arbitrary(target, false);

        Ok(Vec::default())
    }
}

impl_is_pure_true!(setArbitraryStorage_1Call);
impl Cheatcode for setArbitraryStorage_1Call {
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
    >(
        &self,
        ccx: &mut CheatsCtxt<
            '_,
            '_,
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
        let Self { target, overwrite } = self;
        ccx.state
            .arbitrary_storage()
            .mark_arbitrary(target, *overwrite);

        Ok(Vec::default())
    }
}

impl_is_pure_true!(copyStorageCall);
impl Cheatcode for copyStorageCall {
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
    >(
        &self,
        ccx: &mut CheatsCtxt<
            '_,
            '_,
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
        let Self { from, to } = self;

        ensure!(
            !ccx.state.has_arbitrary_storage(to),
            "target address cannot have arbitrary storage"
        );

        if let Ok(from_account) = ccx.ecx.journaled_state.load_account(*from) {
            let from_storage = from_account.storage.clone();
            if ccx.ecx.journaled_state.load_account(*to).is_ok() {
                // SAFETY: We ensured the account was already loaded.
                ccx.ecx.journaled_state.state.get_mut(to).unwrap().storage = from_storage;
                if let Some(arbitrary_storage) = &mut ccx.state.arbitrary_storage {
                    arbitrary_storage.mark_copy(from, to);
                }
            }
        }

        Ok(Vec::default())
    }
}

impl_is_pure_true!(sortCall);
impl Cheatcode for sortCall {
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
    >(
        &self,
        _state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { array } = self;

        let mut sorted_values = array.clone();
        sorted_values.sort();

        Ok(sorted_values.abi_encode())
    }
}

impl_is_pure_false!(shuffleCall);
impl Cheatcode for shuffleCall {
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
        let Self { array } = self;

        let mut shuffled_values = array.clone();
        let rng = state.rng();
        shuffled_values.shuffle(rng);

        Ok(shuffled_values.abi_encode())
    }
}

impl_is_pure_true!(setSeedCall);
impl Cheatcode for setSeedCall {
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
    >(
        &self,
        ccx: &mut CheatsCtxt<
            '_,
            '_,
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
        let Self { seed } = self;
        ccx.state.set_seed(*seed);
        Ok(Vec::default())
    }
}

impl_is_pure_true!(eip712HashType_0Call);
impl Cheatcode for eip712HashType_0Call {
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
            typeNameOrDefinition,
        } = self;

        let type_def = get_canonical_type_def(typeNameOrDefinition, &state.config.eip712_types)?;
        Ok(keccak256(type_def.canonical_definition().as_bytes()).to_vec())
    }
}

impl_is_pure_true!(eip712HashStruct_0Call);
impl Cheatcode for eip712HashStruct_0Call {
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
            typeNameOrDefinition,
            abiEncodedData,
        } = self;

        let type_def = get_canonical_type_def(typeNameOrDefinition, &state.config.eip712_types)?;

        get_struct_hash(&type_def, abiEncodedData)
    }
}

impl_is_pure_true!(eip712HashTypedDataCall);
impl Cheatcode for eip712HashTypedDataCall {
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
    >(
        &self,
        _state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { jsonData } = self;
        let typed_data: TypedData = serde_json::from_str(jsonData)?;
        let digest = typed_data.eip712_signing_hash()?;

        Ok(digest.to_vec())
    }
}

/// Helper to generate a random `uint` value (with given bits or bounded if
/// specified) from type strategy.
fn random_uint<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
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
    bits: Option<U256>,
    bounds: Option<(U256, U256)>,
) -> Result {
    if let Some(bits) = bits {
        // Generate random with specified bits.
        ensure!(bits <= U256::from(256), "number of bits cannot exceed 256");
        return Ok(
            DynSolValue::type_strategy(&DynSolType::Uint(bits.to::<usize>()))
                .new_tree(state.test_runner())
                .unwrap()
                .current()
                .abi_encode(),
        );
    }

    if let Some((min, max)) = bounds {
        ensure!(min <= max, "min must be less than or equal to max");
        // Generate random between range min..=max
        let exclusive_modulo = max - min;
        let mut random_number: U256 = state.rng().random();
        if exclusive_modulo != U256::MAX {
            let inclusive_modulo = exclusive_modulo + U256::from(1);
            random_number %= inclusive_modulo;
        }
        random_number += min;
        return Ok(random_number.abi_encode());
    }

    // Generate random `uint256` value.
    Ok(DynSolValue::type_strategy(&DynSolType::Uint(256))
        .new_tree(state.test_runner())
        .unwrap()
        .current()
        .abi_encode())
}

/// Helper to generate a random `int` value (with given bits if specified) from
/// type strategy.
fn random_int<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
    ChainContextT: ChainContextTr,
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
    bits: Option<U256>,
) -> Result {
    let no_bits = bits.unwrap_or(U256::from(256));
    ensure!(
        no_bits <= U256::from(256),
        "number of bits cannot exceed 256"
    );
    Ok(
        DynSolValue::type_strategy(&DynSolType::Int(no_bits.to::<usize>()))
            .new_tree(state.test_runner())
            .unwrap()
            .current()
            .abi_encode(),
    )
}

/// Resolves an EIP-712 type definition from either:
///
/// - an inline type definition string (detected by the presence of `(`), which
///   is parsed and canonicalized on demand, or
/// - a type name, lazily resolved by parsing the running test contract's
///   Solidity sources via the EIP-712 type provider.
fn get_canonical_type_def(
    name_or_def: &str,
    eip712_types: &Eip712TypeCollection,
) -> Result<Eip712Type> {
    if name_or_def.contains('(') {
        Eip712Type::parse(name_or_def).map_err(|error| fmt_err!("{error}"))
    } else {
        eip712_types
            .get(name_or_def)
            .cloned()
            .map_err(|error| fmt_err!("{error}"))
    }
}

/// Returns the EIP-712 struct hash for provided name, definition and ABI
/// encoded data.
fn get_struct_hash(type_def: &Eip712Type, abi_encoded_data: &Bytes) -> Result {
    let mut resolver = Resolver::default();

    // Populate the resolver by ingesting the canonical type definition, and then
    // get the corresponding `DynSolType` of the primary type.
    resolver
        .ingest_string(type_def.canonical_definition())
        .map_err(|e| fmt_err!("Resolver failed to ingest type definition: {e}"))?;

    let resolved_sol_type = resolver.resolve(type_def.name()).map_err(|e| {
        fmt_err!(
            "Failed to resolve EIP-712 primary type '{}': {e}",
            type_def.name()
        )
    })?;

    // ABI-decode the bytes into `DynSolValue::CustomStruct`.
    let sol_value = resolved_sol_type
        .abi_decode(abi_encoded_data.as_ref())
        .map_err(|e| {
            fmt_err!(
                "Failed to ABI decode using resolved_sol_type directly for '{}': {e}.",
                type_def.name()
            )
        })?;

    // Use the resolver to properly encode the data.
    let encoded_data: Vec<u8> = resolver
        .encode_data(&sol_value)
        .map_err(|e| {
            fmt_err!(
                "Failed to EIP-712 encode data for struct '{}': {e}",
                type_def.name()
            )
        })?
        .ok_or_else(|| {
            fmt_err!(
                "EIP-712 data encoding returned 'None' for struct '{}'",
                type_def.name()
            )
        })?;

    // Compute the type hash of the primary type.
    let type_hash = resolver.type_hash(type_def.name()).map_err(|e| {
        fmt_err!(
            "Failed to compute typeHash for EIP712 type '{}': {e}",
            type_def.name()
        )
    })?;

    // Compute the struct hash of the concatenated type hash and encoded data.
    let mut bytes_to_hash = Vec::with_capacity(32 + encoded_data.len());
    bytes_to_hash.extend_from_slice(type_hash.as_slice());
    bytes_to_hash.extend_from_slice(&encoded_data);

    Ok(keccak256(&bytes_to_hash).to_vec())
}
