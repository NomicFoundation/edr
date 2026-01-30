use std::fmt::Formatter;

use alloy_primitives::{Address, Bytes};
use edr_decoder_revert::RevertDecoder;
use eyre::Report;
use foundry_evm_core::{
    backend::IndeterminismReasons,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use foundry_evm_fuzz::{
    invariant::{FuzzRunIdentifiedContracts, InvariantConfig},
    Reason,
};
use proptest::test_runner::TestError;
use revm::context::result::HaltReasonTr;

use super::{BasicTxDetails, InvariantContract};
use crate::executors::RawCallResult;

/// Stores information about failures and reverts of the invariant tests.
#[derive(Clone, Debug, Default)]
pub struct InvariantFailures {
    /// Total number of reverts.
    pub reverts: usize,
    /// The latest revert reason of a run.
    pub revert_reason: Option<String>,
    /// Maps a broken invariant to its specific error.
    pub error: Option<InvariantFuzzError>,
}

impl InvariantFailures {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_inner(self) -> (usize, Option<InvariantFuzzError>) {
        (self.reverts, self.error)
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum InvariantFuzzError {
    Revert(FailedInvariantCaseData),
    BrokenInvariant(FailedInvariantCaseData),
    MaxAssumeRejects(u32),
    Abi(#[from] alloy_dyn_abi::Error),
    Other(String),
}

impl InvariantFuzzError {}

impl From<eyre::Report> for InvariantFuzzError {
    fn from(value: Report) -> Self {
        Self::Other(value.to_string())
    }
}

impl std::fmt::Display for InvariantFuzzError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(revert_reason) = self.revert_reason() {
            write!(f, "{revert_reason}")
        } else {
            match self {
                InvariantFuzzError::Revert(_) => write!(f, "reverted due to unknown reason"),
                InvariantFuzzError::BrokenInvariant(_) => write!(f, "broken invariant"),
                InvariantFuzzError::MaxAssumeRejects(_) => write!(f, "maximum rejections reached"),
                InvariantFuzzError::Other(error_message) => write!(f, "{error_message}"),
                InvariantFuzzError::Abi(error) => write!(f, "{error}"),
            }
        }
    }
}

impl InvariantFuzzError {
    pub fn indetereminism_reasons(&self) -> Option<IndeterminismReasons> {
        match self {
            Self::BrokenInvariant(case_data) | Self::Revert(case_data) => {
                case_data.indeterminism_reasons.clone()
            }
            Self::Abi(_) | Self::Other(_) | Self::MaxAssumeRejects(_) => None,
        }
    }

    pub fn revert_reason(&self) -> Option<String> {
        match self {
            Self::BrokenInvariant(case_data) | Self::Revert(case_data) => {
                (!case_data.revert_reason.is_empty()).then(|| case_data.revert_reason.clone())
            }
            Self::MaxAssumeRejects(allowed) => Some(format!(
                "`vm.assume` rejected too many inputs ({allowed} allowed)"
            )),
            Self::Abi(_) | Self::Other(_) => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FailedInvariantCaseData {
    /// The proptest error occurred as a result of a test case.
    pub test_error: TestError<Vec<BasicTxDetails>>,
    /// The return reason of the offending call.
    pub return_reason: Reason,
    /// The revert string of the offending call.
    pub revert_reason: String,
    /// Address of the invariant asserter.
    pub addr: Address,
    /// Function calldata for invariant check.
    pub calldata: Bytes,
    /// Inner fuzzing Sequence coming from overriding calls.
    pub inner_sequence: Vec<Option<BasicTxDetails>>,
    /// Shrink run limit
    pub shrink_run_limit: u32,
    /// Fail on revert, used to check sequence when shrinking.
    pub fail_on_revert: bool,
    /// Indeterminism from cheatcodes if any.
    pub indeterminism_reasons: Option<IndeterminismReasons>,
}

impl FailedInvariantCaseData {
    pub fn new<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
    >(
        invariant_contract: &InvariantContract<'_>,
        invariant_config: &InvariantConfig,
        targeted_contracts: &FuzzRunIdentifiedContracts,
        calldata: &[BasicTxDetails],
        call_result: RawCallResult<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
        inner_sequence: &[Option<BasicTxDetails>],
    ) -> Self {
        // Collect abis of fuzzed and invariant contracts to decode custom error.
        let revert_reason = RevertDecoder::new()
            .with_abis(targeted_contracts.targets.lock().values().map(|c| &c.abi))
            .with_abi(invariant_contract.abi)
            .decode(call_result.result.as_ref(), call_result.exit_reason);

        let func = invariant_contract.invariant_function;
        debug_assert!(func.inputs.is_empty());
        let origin = func.name.as_str();
        Self {
            test_error: TestError::Fail(
                format!("{origin}, reason: {revert_reason}").into(),
                calldata.to_vec(),
            ),
            return_reason: "".into(),
            revert_reason,
            addr: invariant_contract.address,
            calldata: func.selector().to_vec().into(),
            inner_sequence: inner_sequence.to_vec(),
            shrink_run_limit: invariant_config.shrink_run_limit,
            fail_on_revert: invariant_config.fail_on_revert,
            indeterminism_reasons: call_result.indeterminism_reasons,
        }
    }
}
