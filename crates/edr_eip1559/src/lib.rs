//! Types related to EIP-1559.

pub use alloy_eips::eip1559::BaseFeeParams as ConstantBaseFeeParams;

/// Possible activation points of different base fee parameters
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaseFeeActivation<HardforkT> {
    /// block number
    BlockNumber(u64),
    /// chain hardfork
    Hardfork(HardforkT),
}
/// A mapping of hardfork to [`ConstantBaseFeeParams`]. This is used to specify
/// dynamic EIP-1559 parameters for chains like OP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DynamicBaseFeeParams<HardforkT> {
    activations: Vec<(BaseFeeActivation<HardforkT>, ConstantBaseFeeParams)>,
}

impl<HardforkT: PartialOrd> DynamicBaseFeeParams<HardforkT> {
    /// Constructs a new instance from the provided mapping.
    pub const fn new(
        activations: Vec<(BaseFeeActivation<HardforkT>, ConstantBaseFeeParams)>,
    ) -> Self {
        Self { activations }
    }

    /// Selects the right [`ConstantBaseFeeParams`] for the given conditions, if
    /// any.
    pub fn at_condition(
        &self,
        hardfork: HardforkT,
        block_number: u64,
    ) -> Option<&ConstantBaseFeeParams> {
        self.activations
            .iter()
            .rev()
            .find(|(activation, _)| match activation {
                BaseFeeActivation::BlockNumber(activation_number) => {
                    *activation_number <= block_number
                }
                BaseFeeActivation::Hardfork(activation_hardfork) => {
                    *activation_hardfork <= hardfork
                }
            })
            .map(|(_, params)| params)
    }
}

/// Type that allows specifying constant or dynamic EIP-1559 parameters based on
/// the active hardfork.
#[derive(Clone, Debug)]
pub enum BaseFeeParams<HardforkT> {
    /// Constant [`ConstantBaseFeeParams`]; used for chains that don't have
    /// dynamic EIP-1559 parameters
    Constant(ConstantBaseFeeParams),
    /// Variable [`ConstantBaseFeeParams`]; used for chains that have dynamic
    /// EIP-1559 parameters like OP
    Dynamic(DynamicBaseFeeParams<HardforkT>),
}

impl<HardforkT: PartialOrd> BaseFeeParams<HardforkT> {
    /// Retrieves the right [`ConstantBaseFeeParams`] for the given conditions,
    /// if any.
    pub fn at_condition(
        &self,
        hardfork: HardforkT,
        block_number: u64,
    ) -> Option<&ConstantBaseFeeParams> {
        match self {
            Self::Constant(params) => Some(params),
            Self::Dynamic(params) => params.at_condition(hardfork, block_number),
        }
    }
}

impl<HardforkT> From<ConstantBaseFeeParams> for BaseFeeParams<HardforkT> {
    fn from(params: ConstantBaseFeeParams) -> Self {
        Self::Constant(params)
    }
}

impl<HardforkT> From<DynamicBaseFeeParams<HardforkT>> for BaseFeeParams<HardforkT> {
    fn from(params: DynamicBaseFeeParams<HardforkT>) -> Self {
        Self::Dynamic(params)
    }
}

#[cfg(test)]
mod tests {
    use alloy_eips::eip1559::{
        BaseFeeParams as ConstantBaseFeeParams, DEFAULT_BASE_FEE_MAX_CHANGE_DENOMINATOR,
        DEFAULT_ELASTICITY_MULTIPLIER,
    };
    use edr_evm_spec::EvmSpecId;

    use crate::{BaseFeeActivation, BaseFeeParams, DynamicBaseFeeParams};

    const LONDON_PARAMS: ConstantBaseFeeParams = ConstantBaseFeeParams {
        max_change_denominator: DEFAULT_BASE_FEE_MAX_CHANGE_DENOMINATOR as u128,
        elasticity_multiplier: DEFAULT_ELASTICITY_MULTIPLIER as u128,
    };

    #[test]
    fn test_variable_base_params_at_condition_respects_order() {
        let prague_params = ConstantBaseFeeParams {
            max_change_denominator: u128::from(DEFAULT_BASE_FEE_MAX_CHANGE_DENOMINATOR),
            elasticity_multiplier: 3,
        };
        let prague_activation_block_number = 22_431_084;
        let base_fee_params = DynamicBaseFeeParams::<EvmSpecId>::new(vec![
            (
                BaseFeeActivation::Hardfork(EvmSpecId::LONDON),
                LONDON_PARAMS,
            ),
            (
                BaseFeeActivation::BlockNumber(prague_activation_block_number),
                prague_params,
            ),
        ]);

        assert_eq!(
            base_fee_params.at_condition(EvmSpecId::LONDON, 12_965_001), /* london activation +
                                                                          * 1 block number */
            Some(&LONDON_PARAMS)
        );
        assert_eq!(
            base_fee_params.at_condition(EvmSpecId::SHANGHAI, 19_426_587), /* shanghai activation + 1 block number */
            Some(&LONDON_PARAMS)
        );
        assert_eq!(
            base_fee_params.at_condition(EvmSpecId::LONDON, prague_activation_block_number + 1), /* london hardfork but prague + 1 block number */
            Some(&prague_params)
        );
    }

    #[test]
    fn test_variable_base_params_at_condition_returns_none_on_missing_config() {
        let base_fee_params = DynamicBaseFeeParams::<EvmSpecId>::new(vec![(
            BaseFeeActivation::Hardfork(EvmSpecId::LONDON),
            LONDON_PARAMS,
        )]);

        assert_eq!(
            base_fee_params.at_condition(EvmSpecId::BERLIN, 12_244_000),
            None
        ); // berlin activation block number
    }

    #[test]
    fn base_fee_params_constant_at_condition_returns_constant_value() {
        let base_fee_params = BaseFeeParams::Constant(LONDON_PARAMS);
        assert_eq!(
            base_fee_params.at_condition(EvmSpecId::FRONTIER, 0),
            Some(&LONDON_PARAMS)
        );
        assert_eq!(
            base_fee_params.at_condition(EvmSpecId::LONDON, 12_965_000),
            Some(&LONDON_PARAMS)
        );
        assert_eq!(
            base_fee_params.at_condition(EvmSpecId::PRAGUE, 22_431_084),
            Some(&LONDON_PARAMS)
        );
    }

    #[test]
    fn base_fee_params_variable_at_condition_returns_variable_behavior() {
        let variable_base_fee_params = DynamicBaseFeeParams::new(vec![(
            BaseFeeActivation::Hardfork(EvmSpecId::LONDON),
            LONDON_PARAMS,
        )]);
        let base_fee_params = BaseFeeParams::Dynamic(variable_base_fee_params.clone());

        assert_eq!(
            base_fee_params.at_condition(EvmSpecId::FRONTIER, 0),
            variable_base_fee_params.at_condition(EvmSpecId::FRONTIER, 0)
        );
        assert_eq!(
            base_fee_params.at_condition(EvmSpecId::LONDON, 12_965_000),
            variable_base_fee_params.at_condition(EvmSpecId::LONDON, 12_965_000)
        );
        assert_eq!(
            base_fee_params.at_condition(EvmSpecId::PRAGUE, 22_431_084),
            variable_base_fee_params.at_condition(EvmSpecId::PRAGUE, 22_431_084)
        );
    }
}
