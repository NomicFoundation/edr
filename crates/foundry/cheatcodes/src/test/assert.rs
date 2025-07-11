use std::fmt::{Debug, Display};

use alloy_primitives::{I256, U256};
use foundry_evm_core::{
    abi::{format_units_int, format_units_uint},
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use itertools::Itertools;
use revm::context::result::HaltReasonTr;

use crate::{
    impl_is_pure_true, Cheatcode, Cheatcodes, Result,
    Vm::{
        assertApproxEqAbsDecimal_0Call, assertApproxEqAbsDecimal_1Call,
        assertApproxEqAbsDecimal_2Call, assertApproxEqAbsDecimal_3Call, assertApproxEqAbs_0Call,
        assertApproxEqAbs_1Call, assertApproxEqAbs_2Call, assertApproxEqAbs_3Call,
        assertApproxEqRelDecimal_0Call, assertApproxEqRelDecimal_1Call,
        assertApproxEqRelDecimal_2Call, assertApproxEqRelDecimal_3Call, assertApproxEqRel_0Call,
        assertApproxEqRel_1Call, assertApproxEqRel_2Call, assertApproxEqRel_3Call,
        assertEqDecimal_0Call, assertEqDecimal_1Call, assertEqDecimal_2Call, assertEqDecimal_3Call,
        assertEq_0Call, assertEq_10Call, assertEq_11Call, assertEq_12Call, assertEq_13Call,
        assertEq_14Call, assertEq_15Call, assertEq_16Call, assertEq_17Call, assertEq_18Call,
        assertEq_19Call, assertEq_1Call, assertEq_20Call, assertEq_21Call, assertEq_22Call,
        assertEq_23Call, assertEq_24Call, assertEq_25Call, assertEq_26Call, assertEq_27Call,
        assertEq_2Call, assertEq_3Call, assertEq_4Call, assertEq_5Call, assertEq_6Call,
        assertEq_7Call, assertEq_8Call, assertEq_9Call, assertFalse_0Call, assertFalse_1Call,
        assertGeDecimal_0Call, assertGeDecimal_1Call, assertGeDecimal_2Call, assertGeDecimal_3Call,
        assertGe_0Call, assertGe_1Call, assertGe_2Call, assertGe_3Call, assertGtDecimal_0Call,
        assertGtDecimal_1Call, assertGtDecimal_2Call, assertGtDecimal_3Call, assertGt_0Call,
        assertGt_1Call, assertGt_2Call, assertGt_3Call, assertLeDecimal_0Call,
        assertLeDecimal_1Call, assertLeDecimal_2Call, assertLeDecimal_3Call, assertLe_0Call,
        assertLe_1Call, assertLe_2Call, assertLe_3Call, assertLtDecimal_0Call,
        assertLtDecimal_1Call, assertLtDecimal_2Call, assertLtDecimal_3Call, assertLt_0Call,
        assertLt_1Call, assertLt_2Call, assertLt_3Call, assertNotEqDecimal_0Call,
        assertNotEqDecimal_1Call, assertNotEqDecimal_2Call, assertNotEqDecimal_3Call,
        assertNotEq_0Call, assertNotEq_10Call, assertNotEq_11Call, assertNotEq_12Call,
        assertNotEq_13Call, assertNotEq_14Call, assertNotEq_15Call, assertNotEq_16Call,
        assertNotEq_17Call, assertNotEq_18Call, assertNotEq_19Call, assertNotEq_1Call,
        assertNotEq_20Call, assertNotEq_21Call, assertNotEq_22Call, assertNotEq_23Call,
        assertNotEq_24Call, assertNotEq_25Call, assertNotEq_26Call, assertNotEq_27Call,
        assertNotEq_2Call, assertNotEq_3Call, assertNotEq_4Call, assertNotEq_5Call,
        assertNotEq_6Call, assertNotEq_7Call, assertNotEq_8Call, assertNotEq_9Call,
        assertTrue_0Call, assertTrue_1Call,
    },
};

const EQ_REL_DELTA_RESOLUTION: U256 = U256::from_limbs([18, 0, 0, 0]);

#[derive(Debug, thiserror::Error)]
#[error("assertion failed")]
struct SimpleAssertionError;

#[derive(thiserror::Error, Debug)]
enum ComparisonAssertionError<'a, T> {
    Ne { left: &'a T, right: &'a T },
    Eq { left: &'a T, right: &'a T },
    Ge { left: &'a T, right: &'a T },
    Gt { left: &'a T, right: &'a T },
    Le { left: &'a T, right: &'a T },
    Lt { left: &'a T, right: &'a T },
}

macro_rules! format_values {
    ($self:expr, $format_fn:expr) => {
        match $self {
            Self::Ne { left, right } => format!("{} == {}", $format_fn(left), $format_fn(right)),
            Self::Eq { left, right } => format!("{} != {}", $format_fn(left), $format_fn(right)),
            Self::Ge { left, right } => format!("{} < {}", $format_fn(left), $format_fn(right)),
            Self::Gt { left, right } => format!("{} <= {}", $format_fn(left), $format_fn(right)),
            Self::Le { left, right } => format!("{} > {}", $format_fn(left), $format_fn(right)),
            Self::Lt { left, right } => format!("{} >= {}", $format_fn(left), $format_fn(right)),
        }
    };
}

impl<T: Display> ComparisonAssertionError<'_, T> {
    fn format_for_values(&self) -> String {
        format_values!(self, T::to_string)
    }
}

impl<T: Display> ComparisonAssertionError<'_, Vec<T>> {
    fn format_for_arrays(&self) -> String {
        let formatter = |v: &Vec<T>| format!("[{}]", v.iter().format(", "));
        format_values!(self, formatter)
    }
}

impl ComparisonAssertionError<'_, U256> {
    fn format_with_decimals(&self, decimals: &U256) -> String {
        let formatter = |v: &U256| format_units_uint(v, decimals);
        format_values!(self, formatter)
    }
}

impl ComparisonAssertionError<'_, I256> {
    fn format_with_decimals(&self, decimals: &U256) -> String {
        let formatter = |v: &I256| format_units_int(v, decimals);
        format_values!(self, formatter)
    }
}

#[derive(thiserror::Error, Debug)]
#[error("{left} !~= {right} (max delta: {max_delta}, real delta: {real_delta})")]
struct EqAbsAssertionError<T, D> {
    left: T,
    right: T,
    max_delta: D,
    real_delta: D,
}

impl EqAbsAssertionError<U256, U256> {
    fn format_with_decimals(&self, decimals: &U256) -> String {
        format!(
            "{} !~= {} (max delta: {}, real delta: {})",
            format_units_uint(&self.left, decimals),
            format_units_uint(&self.right, decimals),
            format_units_uint(&self.max_delta, decimals),
            format_units_uint(&self.real_delta, decimals),
        )
    }
}

impl EqAbsAssertionError<I256, U256> {
    fn format_with_decimals(&self, decimals: &U256) -> String {
        format!(
            "{} !~= {} (max delta: {}, real delta: {})",
            format_units_int(&self.left, decimals),
            format_units_int(&self.right, decimals),
            format_units_uint(&self.max_delta, decimals),
            format_units_uint(&self.real_delta, decimals),
        )
    }
}

fn format_delta_percent(delta: &U256) -> String {
    format!(
        "{}%",
        format_units_uint(delta, &(EQ_REL_DELTA_RESOLUTION - U256::from(2)))
    )
}

#[derive(Debug)]
enum EqRelDelta {
    Defined(U256),
    Undefined,
}

impl Display for EqRelDelta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Defined(delta) => write!(f, "{}", format_delta_percent(delta)),
            Self::Undefined => write!(f, "undefined"),
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error(
    "{left} !~= {right} (max delta: {}, real delta: {})",
    format_delta_percent(max_delta),
    real_delta
)]
struct EqRelAssertionFailure<T> {
    left: T,
    right: T,
    max_delta: U256,
    real_delta: EqRelDelta,
}

#[derive(thiserror::Error, Debug)]
enum EqRelAssertionError<T> {
    #[error(transparent)]
    Failure(Box<EqRelAssertionFailure<T>>),
    #[error("overflow in delta calculation")]
    Overflow,
}

impl EqRelAssertionError<U256> {
    fn format_with_decimals(&self, decimals: &U256) -> String {
        match self {
            Self::Failure(f) => format!(
                "{} !~= {} (max delta: {}, real delta: {})",
                format_units_uint(&f.left, decimals),
                format_units_uint(&f.right, decimals),
                format_delta_percent(&f.max_delta),
                &f.real_delta,
            ),
            Self::Overflow => self.to_string(),
        }
    }
}

impl EqRelAssertionError<I256> {
    fn format_with_decimals(&self, decimals: &U256) -> String {
        match self {
            Self::Failure(f) => format!(
                "{} !~= {} (max delta: {}, real delta: {})",
                format_units_int(&f.left, decimals),
                format_units_int(&f.right, decimals),
                format_delta_percent(&f.max_delta),
                &f.real_delta,
            ),
            Self::Overflow => self.to_string(),
        }
    }
}

type ComparisonResult<'a, T> = Result<Vec<u8>, ComparisonAssertionError<'a, T>>;

impl_is_pure_true!(assertTrue_0Call);
impl Cheatcode for assertTrue_0Call {
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
        Ok(assert_true(self.condition).map_err(|e| e.to_string())?)
    }
}

impl_is_pure_true!(assertTrue_1Call);
impl Cheatcode for assertTrue_1Call {
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
        Ok(assert_true(self.condition).map_err(|_err| self.error.clone())?)
    }
}

impl_is_pure_true!(assertFalse_0Call);
impl Cheatcode for assertFalse_0Call {
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
        Ok(assert_false(self.condition).map_err(|e| e.to_string())?)
    }
}

impl_is_pure_true!(assertFalse_1Call);
impl Cheatcode for assertFalse_1Call {
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
        Ok(assert_false(self.condition).map_err(|_err| self.error.clone())?)
    }
}

impl_is_pure_true!(assertEq_0Call);
impl Cheatcode for assertEq_0Call {
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
        let Self { left, right } = self;
        Ok(assert_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertEq_1Call);
impl Cheatcode for assertEq_1Call {
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
        let Self { left, right, error } = self;
        Ok(assert_eq(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertEq_2Call);
impl Cheatcode for assertEq_2Call {
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
        let Self { left, right } = self;
        Ok(assert_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertEq_3Call);
impl Cheatcode for assertEq_3Call {
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
        let Self { left, right, error } = self;
        Ok(assert_eq(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertEq_4Call);
impl Cheatcode for assertEq_4Call {
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
        let Self { left, right } = self;
        Ok(assert_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertEq_5Call);
impl Cheatcode for assertEq_5Call {
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
        let Self { left, right, error } = self;
        Ok(assert_eq(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertEq_6Call);
impl Cheatcode for assertEq_6Call {
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
        let Self { left, right } = self;
        Ok(assert_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertEq_7Call);
impl Cheatcode for assertEq_7Call {
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
        let Self { left, right, error } = self;
        Ok(assert_eq(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertEq_8Call);
impl Cheatcode for assertEq_8Call {
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
        let Self { left, right } = self;
        Ok(assert_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertEq_9Call);
impl Cheatcode for assertEq_9Call {
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
        let Self { left, right, error } = self;
        Ok(assert_eq(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertEq_10Call);
impl Cheatcode for assertEq_10Call {
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
        let Self { left, right } = self;
        Ok(assert_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertEq_11Call);
impl Cheatcode for assertEq_11Call {
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
        let Self { left, right, error } = self;
        Ok(assert_eq(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertEq_12Call);
impl Cheatcode for assertEq_12Call {
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
        let Self { left, right } = self;
        Ok(
            assert_eq(&hex::encode_prefixed(left), &hex::encode_prefixed(right))
                .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?,
        )
    }
}

impl_is_pure_true!(assertEq_13Call);
impl Cheatcode for assertEq_13Call {
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
        let Self { left, right, error } = self;
        Ok(
            assert_eq(&hex::encode_prefixed(left), &hex::encode_prefixed(right))
                .map_err(|e| format!("{}: {}", error, e.format_for_values()))?,
        )
    }
}

impl_is_pure_true!(assertEq_14Call);
impl Cheatcode for assertEq_14Call {
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
        let Self { left, right } = self;
        Ok(assert_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_15Call);
impl Cheatcode for assertEq_15Call {
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
        let Self { left, right, error } = self;
        Ok(assert_eq(left, right).map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_16Call);
impl Cheatcode for assertEq_16Call {
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
        let Self { left, right } = self;
        Ok(assert_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_17Call);
impl Cheatcode for assertEq_17Call {
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
        let Self { left, right, error } = self;
        Ok(assert_eq(left, right).map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_18Call);
impl Cheatcode for assertEq_18Call {
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
        let Self { left, right } = self;
        Ok(assert_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_19Call);
impl Cheatcode for assertEq_19Call {
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
        let Self { left, right, error } = self;
        Ok(assert_eq(left, right).map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_20Call);
impl Cheatcode for assertEq_20Call {
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
        let Self { left, right } = self;
        Ok(assert_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_21Call);
impl Cheatcode for assertEq_21Call {
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
        let Self { left, right, error } = self;
        Ok(assert_eq(left, right).map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_22Call);
impl Cheatcode for assertEq_22Call {
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
        let Self { left, right } = self;
        Ok(assert_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_23Call);
impl Cheatcode for assertEq_23Call {
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
        let Self { left, right, error } = self;
        Ok(assert_eq(left, right).map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_24Call);
impl Cheatcode for assertEq_24Call {
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
        let Self { left, right } = self;
        Ok(assert_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_25Call);
impl Cheatcode for assertEq_25Call {
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
        let Self { left, right, error } = self;
        Ok(assert_eq(left, right).map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_26Call);
impl Cheatcode for assertEq_26Call {
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
        let Self { left, right } = self;
        let left = left.iter().map(hex::encode_prefixed).collect::<Vec<_>>();
        let right = right.iter().map(hex::encode_prefixed).collect::<Vec<_>>();
        Ok(assert_eq(&left, &right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertEq_27Call);
impl Cheatcode for assertEq_27Call {
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
        let Self { left, right, error } = self;
        let left = left.iter().map(hex::encode_prefixed).collect::<Vec<_>>();
        let right = right.iter().map(hex::encode_prefixed).collect::<Vec<_>>();
        Ok(
            assert_eq(&left, &right)
                .map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?,
        )
    }
}

impl_is_pure_true!(assertEqDecimal_0Call);
impl Cheatcode for assertEqDecimal_0Call {
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
        Ok(assert_eq(&self.left, &self.right).map_err(|e| {
            format!(
                "assertion failed: {}",
                e.format_with_decimals(&self.decimals)
            )
        })?)
    }
}

impl_is_pure_true!(assertEqDecimal_1Call);
impl Cheatcode for assertEqDecimal_1Call {
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
        Ok(assert_eq(&self.left, &self.right)
            .map_err(|e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)))?)
    }
}

impl_is_pure_true!(assertEqDecimal_2Call);
impl Cheatcode for assertEqDecimal_2Call {
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
        Ok(assert_eq(&self.left, &self.right).map_err(|e| {
            format!(
                "assertion failed: {}",
                e.format_with_decimals(&self.decimals)
            )
        })?)
    }
}

impl_is_pure_true!(assertEqDecimal_3Call);
impl Cheatcode for assertEqDecimal_3Call {
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
        Ok(assert_eq(&self.left, &self.right)
            .map_err(|e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)))?)
    }
}

impl_is_pure_true!(assertNotEq_0Call);
impl Cheatcode for assertNotEq_0Call {
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
        let Self { left, right } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertNotEq_1Call);
impl Cheatcode for assertNotEq_1Call {
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
        let Self { left, right, error } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertNotEq_2Call);
impl Cheatcode for assertNotEq_2Call {
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
        let Self { left, right } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertNotEq_3Call);
impl Cheatcode for assertNotEq_3Call {
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
        let Self { left, right, error } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertNotEq_4Call);
impl Cheatcode for assertNotEq_4Call {
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
        let Self { left, right } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertNotEq_5Call);
impl Cheatcode for assertNotEq_5Call {
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
        let Self { left, right, error } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertNotEq_6Call);
impl Cheatcode for assertNotEq_6Call {
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
        let Self { left, right } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertNotEq_7Call);
impl Cheatcode for assertNotEq_7Call {
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
        let Self { left, right, error } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertNotEq_8Call);
impl Cheatcode for assertNotEq_8Call {
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
        let Self { left, right } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertNotEq_9Call);
impl Cheatcode for assertNotEq_9Call {
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
        let Self { left, right, error } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertNotEq_10Call);
impl Cheatcode for assertNotEq_10Call {
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
        let Self { left, right } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertNotEq_11Call);
impl Cheatcode for assertNotEq_11Call {
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
        let Self { left, right, error } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertNotEq_12Call);
impl Cheatcode for assertNotEq_12Call {
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
        let Self { left, right } = self;
        Ok(
            assert_not_eq(&hex::encode_prefixed(left), &hex::encode_prefixed(right))
                .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?,
        )
    }
}

impl_is_pure_true!(assertNotEq_13Call);
impl Cheatcode for assertNotEq_13Call {
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
        let Self { left, right, error } = self;
        Ok(
            assert_not_eq(&hex::encode_prefixed(left), &hex::encode_prefixed(right))
                .map_err(|e| format!("{}: {}", error, e.format_for_values()))?,
        )
    }
}

impl_is_pure_true!(assertNotEq_14Call);
impl Cheatcode for assertNotEq_14Call {
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
        let Self { left, right } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_15Call);
impl Cheatcode for assertNotEq_15Call {
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
        let Self { left, right, error } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_16Call);
impl Cheatcode for assertNotEq_16Call {
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
        let Self { left, right } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_17Call);
impl Cheatcode for assertNotEq_17Call {
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
        let Self { left, right, error } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_18Call);
impl Cheatcode for assertNotEq_18Call {
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
        let Self { left, right } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_19Call);
impl Cheatcode for assertNotEq_19Call {
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
        let Self { left, right, error } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_20Call);
impl Cheatcode for assertNotEq_20Call {
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
        let Self { left, right } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_21Call);
impl Cheatcode for assertNotEq_21Call {
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
        let Self { left, right, error } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_22Call);
impl Cheatcode for assertNotEq_22Call {
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
        let Self { left, right } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_23Call);
impl Cheatcode for assertNotEq_23Call {
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
        let Self { left, right, error } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_24Call);
impl Cheatcode for assertNotEq_24Call {
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
        let Self { left, right } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_25Call);
impl Cheatcode for assertNotEq_25Call {
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
        let Self { left, right, error } = self;
        Ok(assert_not_eq(left, right)
            .map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_26Call);
impl Cheatcode for assertNotEq_26Call {
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
        let Self { left, right } = self;
        let left = left.iter().map(hex::encode_prefixed).collect::<Vec<_>>();
        let right = right.iter().map(hex::encode_prefixed).collect::<Vec<_>>();
        Ok(assert_not_eq(&left, &right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEq_27Call);
impl Cheatcode for assertNotEq_27Call {
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
        let Self { left, right, error } = self;
        let left = left.iter().map(hex::encode_prefixed).collect::<Vec<_>>();
        let right = right.iter().map(hex::encode_prefixed).collect::<Vec<_>>();
        Ok(assert_not_eq(&left, &right)
            .map_err(|e| format!("{}: {}", error, e.format_for_arrays()))?)
    }
}

impl_is_pure_true!(assertNotEqDecimal_0Call);
impl Cheatcode for assertNotEqDecimal_0Call {
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
        Ok(assert_not_eq(&self.left, &self.right).map_err(|e| {
            format!(
                "assertion failed: {}",
                e.format_with_decimals(&self.decimals)
            )
        })?)
    }
}

impl_is_pure_true!(assertNotEqDecimal_1Call);
impl Cheatcode for assertNotEqDecimal_1Call {
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
        Ok(assert_not_eq(&self.left, &self.right)
            .map_err(|e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)))?)
    }
}

impl_is_pure_true!(assertNotEqDecimal_2Call);
impl Cheatcode for assertNotEqDecimal_2Call {
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
        Ok(assert_not_eq(&self.left, &self.right).map_err(|e| {
            format!(
                "assertion failed: {}",
                e.format_with_decimals(&self.decimals)
            )
        })?)
    }
}

impl_is_pure_true!(assertNotEqDecimal_3Call);
impl Cheatcode for assertNotEqDecimal_3Call {
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
        Ok(assert_not_eq(&self.left, &self.right)
            .map_err(|e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)))?)
    }
}

impl_is_pure_true!(assertGt_0Call);
impl Cheatcode for assertGt_0Call {
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
        let Self { left, right } = self;
        Ok(assert_gt(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertGt_1Call);
impl Cheatcode for assertGt_1Call {
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
        let Self { left, right, error } = self;
        Ok(assert_gt(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertGt_2Call);
impl Cheatcode for assertGt_2Call {
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
        let Self { left, right } = self;
        Ok(assert_gt(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertGt_3Call);
impl Cheatcode for assertGt_3Call {
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
        let Self { left, right, error } = self;
        Ok(assert_gt(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertGtDecimal_0Call);
impl Cheatcode for assertGtDecimal_0Call {
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
        Ok(assert_gt(&self.left, &self.right).map_err(|e| {
            format!(
                "assertion failed: {}",
                e.format_with_decimals(&self.decimals)
            )
        })?)
    }
}

impl_is_pure_true!(assertGtDecimal_1Call);
impl Cheatcode for assertGtDecimal_1Call {
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
        Ok(assert_gt(&self.left, &self.right)
            .map_err(|e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)))?)
    }
}

impl_is_pure_true!(assertGtDecimal_2Call);
impl Cheatcode for assertGtDecimal_2Call {
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
        Ok(assert_gt(&self.left, &self.right).map_err(|e| {
            format!(
                "assertion failed: {}",
                e.format_with_decimals(&self.decimals)
            )
        })?)
    }
}

impl_is_pure_true!(assertGtDecimal_3Call);
impl Cheatcode for assertGtDecimal_3Call {
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
        Ok(assert_gt(&self.left, &self.right)
            .map_err(|e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)))?)
    }
}

impl_is_pure_true!(assertGe_0Call);
impl Cheatcode for assertGe_0Call {
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
        let Self { left, right } = self;
        Ok(assert_ge(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertGe_1Call);
impl Cheatcode for assertGe_1Call {
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
        let Self { left, right, error } = self;
        Ok(assert_ge(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertGe_2Call);
impl Cheatcode for assertGe_2Call {
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
        let Self { left, right } = self;
        Ok(assert_ge(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertGe_3Call);
impl Cheatcode for assertGe_3Call {
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
        let Self { left, right, error } = self;
        Ok(assert_ge(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertGeDecimal_0Call);
impl Cheatcode for assertGeDecimal_0Call {
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
        Ok(assert_ge(&self.left, &self.right).map_err(|e| {
            format!(
                "assertion failed: {}",
                e.format_with_decimals(&self.decimals)
            )
        })?)
    }
}

impl_is_pure_true!(assertGeDecimal_1Call);
impl Cheatcode for assertGeDecimal_1Call {
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
        Ok(assert_ge(&self.left, &self.right)
            .map_err(|e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)))?)
    }
}

impl_is_pure_true!(assertGeDecimal_2Call);
impl Cheatcode for assertGeDecimal_2Call {
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
        Ok(assert_ge(&self.left, &self.right).map_err(|e| {
            format!(
                "assertion failed: {}",
                e.format_with_decimals(&self.decimals)
            )
        })?)
    }
}

impl_is_pure_true!(assertGeDecimal_3Call);
impl Cheatcode for assertGeDecimal_3Call {
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
        Ok(assert_ge(&self.left, &self.right)
            .map_err(|e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)))?)
    }
}

impl_is_pure_true!(assertLt_0Call);
impl Cheatcode for assertLt_0Call {
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
        let Self { left, right } = self;
        Ok(assert_lt(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertLt_1Call);
impl Cheatcode for assertLt_1Call {
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
        let Self { left, right, error } = self;
        Ok(assert_lt(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertLt_2Call);
impl Cheatcode for assertLt_2Call {
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
        let Self { left, right } = self;
        Ok(assert_lt(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertLt_3Call);
impl Cheatcode for assertLt_3Call {
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
        let Self { left, right, error } = self;
        Ok(assert_lt(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertLtDecimal_0Call);
impl Cheatcode for assertLtDecimal_0Call {
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
        Ok(assert_lt(&self.left, &self.right).map_err(|e| {
            format!(
                "assertion failed: {}",
                e.format_with_decimals(&self.decimals)
            )
        })?)
    }
}

impl_is_pure_true!(assertLtDecimal_1Call);
impl Cheatcode for assertLtDecimal_1Call {
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
        Ok(assert_lt(&self.left, &self.right)
            .map_err(|e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)))?)
    }
}

impl_is_pure_true!(assertLtDecimal_2Call);
impl Cheatcode for assertLtDecimal_2Call {
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
        Ok(assert_lt(&self.left, &self.right).map_err(|e| {
            format!(
                "assertion failed: {}",
                e.format_with_decimals(&self.decimals)
            )
        })?)
    }
}

impl_is_pure_true!(assertLtDecimal_3Call);
impl Cheatcode for assertLtDecimal_3Call {
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
        Ok(assert_lt(&self.left, &self.right)
            .map_err(|e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)))?)
    }
}

impl_is_pure_true!(assertLe_0Call);
impl Cheatcode for assertLe_0Call {
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
        let Self { left, right } = self;
        Ok(assert_le(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertLe_1Call);
impl Cheatcode for assertLe_1Call {
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
        let Self { left, right, error } = self;
        Ok(assert_le(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertLe_2Call);
impl Cheatcode for assertLe_2Call {
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
        let Self { left, right } = self;
        Ok(assert_le(left, right)
            .map_err(|e| format!("assertion failed: {}", e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertLe_3Call);
impl Cheatcode for assertLe_3Call {
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
        let Self { left, right, error } = self;
        Ok(assert_le(left, right).map_err(|e| format!("{}: {}", error, e.format_for_values()))?)
    }
}

impl_is_pure_true!(assertLeDecimal_0Call);
impl Cheatcode for assertLeDecimal_0Call {
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
        Ok(assert_le(&self.left, &self.right).map_err(|e| {
            format!(
                "assertion failed: {}",
                e.format_with_decimals(&self.decimals)
            )
        })?)
    }
}

impl_is_pure_true!(assertLeDecimal_1Call);
impl Cheatcode for assertLeDecimal_1Call {
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
        Ok(assert_le(&self.left, &self.right)
            .map_err(|e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)))?)
    }
}

impl_is_pure_true!(assertLeDecimal_2Call);
impl Cheatcode for assertLeDecimal_2Call {
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
        Ok(assert_le(&self.left, &self.right).map_err(|e| {
            format!(
                "assertion failed: {}",
                e.format_with_decimals(&self.decimals)
            )
        })?)
    }
}

impl_is_pure_true!(assertLeDecimal_3Call);
impl Cheatcode for assertLeDecimal_3Call {
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
        Ok(assert_le(&self.left, &self.right)
            .map_err(|e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)))?)
    }
}

impl_is_pure_true!(assertApproxEqAbs_0Call);
impl Cheatcode for assertApproxEqAbs_0Call {
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
        Ok(
            uint_assert_approx_eq_abs(self.left, self.right, self.maxDelta)
                .map_err(|e| format!("assertion failed: {e}"))?,
        )
    }
}

impl_is_pure_true!(assertApproxEqAbs_1Call);
impl Cheatcode for assertApproxEqAbs_1Call {
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
        Ok(
            uint_assert_approx_eq_abs(self.left, self.right, self.maxDelta)
                .map_err(|e| format!("{}: {}", self.error, e))?,
        )
    }
}

impl_is_pure_true!(assertApproxEqAbs_2Call);
impl Cheatcode for assertApproxEqAbs_2Call {
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
        Ok(
            int_assert_approx_eq_abs(self.left, self.right, self.maxDelta)
                .map_err(|e| format!("assertion failed: {e}"))?,
        )
    }
}

impl_is_pure_true!(assertApproxEqAbs_3Call);
impl Cheatcode for assertApproxEqAbs_3Call {
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
        Ok(
            int_assert_approx_eq_abs(self.left, self.right, self.maxDelta)
                .map_err(|e| format!("{}: {}", self.error, e))?,
        )
    }
}

impl_is_pure_true!(assertApproxEqAbsDecimal_0Call);
impl Cheatcode for assertApproxEqAbsDecimal_0Call {
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
        Ok(
            uint_assert_approx_eq_abs(self.left, self.right, self.maxDelta).map_err(|e| {
                format!(
                    "assertion failed: {}",
                    e.format_with_decimals(&self.decimals)
                )
            })?,
        )
    }
}

impl_is_pure_true!(assertApproxEqAbsDecimal_1Call);
impl Cheatcode for assertApproxEqAbsDecimal_1Call {
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
        Ok(
            uint_assert_approx_eq_abs(self.left, self.right, self.maxDelta).map_err(|e| {
                format!("{}: {}", self.error, e.format_with_decimals(&self.decimals))
            })?,
        )
    }
}

impl_is_pure_true!(assertApproxEqAbsDecimal_2Call);
impl Cheatcode for assertApproxEqAbsDecimal_2Call {
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
        Ok(
            int_assert_approx_eq_abs(self.left, self.right, self.maxDelta).map_err(|e| {
                format!(
                    "assertion failed: {}",
                    e.format_with_decimals(&self.decimals)
                )
            })?,
        )
    }
}

impl_is_pure_true!(assertApproxEqAbsDecimal_3Call);
impl Cheatcode for assertApproxEqAbsDecimal_3Call {
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
        Ok(
            int_assert_approx_eq_abs(self.left, self.right, self.maxDelta).map_err(|e| {
                format!("{}: {}", self.error, e.format_with_decimals(&self.decimals))
            })?,
        )
    }
}

impl_is_pure_true!(assertApproxEqRel_0Call);
impl Cheatcode for assertApproxEqRel_0Call {
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
        Ok(
            uint_assert_approx_eq_rel(self.left, self.right, self.maxPercentDelta)
                .map_err(|e| format!("assertion failed: {e}"))?,
        )
    }
}

impl_is_pure_true!(assertApproxEqRel_1Call);
impl Cheatcode for assertApproxEqRel_1Call {
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
        Ok(
            uint_assert_approx_eq_rel(self.left, self.right, self.maxPercentDelta)
                .map_err(|e| format!("{}: {}", self.error, e))?,
        )
    }
}

impl_is_pure_true!(assertApproxEqRel_2Call);
impl Cheatcode for assertApproxEqRel_2Call {
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
        Ok(
            int_assert_approx_eq_rel(self.left, self.right, self.maxPercentDelta)
                .map_err(|e| format!("assertion failed: {e}"))?,
        )
    }
}

impl_is_pure_true!(assertApproxEqRel_3Call);
impl Cheatcode for assertApproxEqRel_3Call {
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
        Ok(
            int_assert_approx_eq_rel(self.left, self.right, self.maxPercentDelta)
                .map_err(|e| format!("{}: {}", self.error, e))?,
        )
    }
}

impl_is_pure_true!(assertApproxEqRelDecimal_0Call);
impl Cheatcode for assertApproxEqRelDecimal_0Call {
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
        Ok(
            uint_assert_approx_eq_rel(self.left, self.right, self.maxPercentDelta).map_err(
                |e| {
                    format!(
                        "assertion failed: {}",
                        e.format_with_decimals(&self.decimals)
                    )
                },
            )?,
        )
    }
}

impl_is_pure_true!(assertApproxEqRelDecimal_1Call);
impl Cheatcode for assertApproxEqRelDecimal_1Call {
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
        Ok(
            uint_assert_approx_eq_rel(self.left, self.right, self.maxPercentDelta).map_err(
                |e| format!("{}: {}", self.error, e.format_with_decimals(&self.decimals)),
            )?,
        )
    }
}

impl_is_pure_true!(assertApproxEqRelDecimal_2Call);
impl Cheatcode for assertApproxEqRelDecimal_2Call {
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
        Ok(
            int_assert_approx_eq_rel(self.left, self.right, self.maxPercentDelta).map_err(|e| {
                format!(
                    "assertion failed: {}",
                    e.format_with_decimals(&self.decimals)
                )
            })?,
        )
    }
}

impl_is_pure_true!(assertApproxEqRelDecimal_3Call);
impl Cheatcode for assertApproxEqRelDecimal_3Call {
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
        Ok(
            int_assert_approx_eq_rel(self.left, self.right, self.maxPercentDelta).map_err(|e| {
                format!("{}: {}", self.error, e.format_with_decimals(&self.decimals))
            })?,
        )
    }
}

fn assert_true(condition: bool) -> Result<Vec<u8>, SimpleAssertionError> {
    if condition {
        Ok(Vec::default())
    } else {
        Err(SimpleAssertionError)
    }
}

fn assert_false(condition: bool) -> Result<Vec<u8>, SimpleAssertionError> {
    if !condition {
        Ok(Vec::default())
    } else {
        Err(SimpleAssertionError)
    }
}

fn assert_eq<'a, T: PartialEq>(left: &'a T, right: &'a T) -> ComparisonResult<'a, T> {
    if left == right {
        Ok(Vec::default())
    } else {
        Err(ComparisonAssertionError::Eq { left, right })
    }
}

fn assert_not_eq<'a, T: PartialEq>(left: &'a T, right: &'a T) -> ComparisonResult<'a, T> {
    if left != right {
        Ok(Vec::default())
    } else {
        Err(ComparisonAssertionError::Ne { left, right })
    }
}

fn get_delta_uint(left: U256, right: U256) -> U256 {
    if left > right {
        left - right
    } else {
        right - left
    }
}

fn get_delta_int(left: I256, right: I256) -> U256 {
    let (left_sign, left_abs) = left.into_sign_and_abs();
    let (right_sign, right_abs) = right.into_sign_and_abs();

    if left_sign == right_sign {
        if left_abs > right_abs {
            left_abs - right_abs
        } else {
            right_abs - left_abs
        }
    } else {
        left_abs + right_abs
    }
}

fn uint_assert_approx_eq_abs(
    left: U256,
    right: U256,
    max_delta: U256,
) -> Result<Vec<u8>, Box<EqAbsAssertionError<U256, U256>>> {
    let delta = get_delta_uint(left, right);

    if delta <= max_delta {
        Ok(Vec::default())
    } else {
        Err(Box::new(EqAbsAssertionError {
            left,
            right,
            max_delta,
            real_delta: delta,
        }))
    }
}

fn int_assert_approx_eq_abs(
    left: I256,
    right: I256,
    max_delta: U256,
) -> Result<Vec<u8>, Box<EqAbsAssertionError<I256, U256>>> {
    let delta = get_delta_int(left, right);

    if delta <= max_delta {
        Ok(Vec::default())
    } else {
        Err(Box::new(EqAbsAssertionError {
            left,
            right,
            max_delta,
            real_delta: delta,
        }))
    }
}

fn uint_assert_approx_eq_rel(
    left: U256,
    right: U256,
    max_delta: U256,
) -> Result<Vec<u8>, EqRelAssertionError<U256>> {
    if right.is_zero() {
        if left.is_zero() {
            return Ok(Vec::default());
        } else {
            return Err(EqRelAssertionError::Failure(Box::new(
                EqRelAssertionFailure {
                    left,
                    right,
                    max_delta,
                    real_delta: EqRelDelta::Undefined,
                },
            )));
        };
    }

    let delta = get_delta_uint(left, right)
        .checked_mul(U256::pow(U256::from(10), EQ_REL_DELTA_RESOLUTION))
        .ok_or(EqRelAssertionError::Overflow)?
        / right;

    if delta <= max_delta {
        Ok(Vec::default())
    } else {
        Err(EqRelAssertionError::Failure(Box::new(
            EqRelAssertionFailure {
                left,
                right,
                max_delta,
                real_delta: EqRelDelta::Defined(delta),
            },
        )))
    }
}

fn int_assert_approx_eq_rel(
    left: I256,
    right: I256,
    max_delta: U256,
) -> Result<Vec<u8>, EqRelAssertionError<I256>> {
    if right.is_zero() {
        if left.is_zero() {
            return Ok(Vec::default());
        } else {
            return Err(EqRelAssertionError::Failure(Box::new(
                EqRelAssertionFailure {
                    left,
                    right,
                    max_delta,
                    real_delta: EqRelDelta::Undefined,
                },
            )));
        }
    }

    let (_, abs_right) = right.into_sign_and_abs();
    let delta = get_delta_int(left, right)
        .checked_mul(U256::pow(U256::from(10), EQ_REL_DELTA_RESOLUTION))
        .ok_or(EqRelAssertionError::Overflow)?
        / abs_right;

    if delta <= max_delta {
        Ok(Vec::default())
    } else {
        Err(EqRelAssertionError::Failure(Box::new(
            EqRelAssertionFailure {
                left,
                right,
                max_delta,
                real_delta: EqRelDelta::Defined(delta),
            },
        )))
    }
}

fn assert_gt<'a, T: PartialOrd>(left: &'a T, right: &'a T) -> ComparisonResult<'a, T> {
    if left > right {
        Ok(Vec::default())
    } else {
        Err(ComparisonAssertionError::Gt { left, right })
    }
}

fn assert_ge<'a, T: PartialOrd>(left: &'a T, right: &'a T) -> ComparisonResult<'a, T> {
    if left >= right {
        Ok(Vec::default())
    } else {
        Err(ComparisonAssertionError::Ge { left, right })
    }
}

fn assert_lt<'a, T: PartialOrd>(left: &'a T, right: &'a T) -> ComparisonResult<'a, T> {
    if left < right {
        Ok(Vec::default())
    } else {
        Err(ComparisonAssertionError::Lt { left, right })
    }
}

fn assert_le<'a, T: PartialOrd>(left: &'a T, right: &'a T) -> ComparisonResult<'a, T> {
    if left <= right {
        Ok(Vec::default())
    } else {
        Err(ComparisonAssertionError::Le { left, right })
    }
}
