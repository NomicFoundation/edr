pub use alloy_eips::eip1559::BaseFeeParams as ConstantBaseFeeParams;
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// Criteria for indicating the different activation of different base fee parameters
pub enum DynamicBaseFeeCondition<HardforkT> {
    /// block number
    BlockNumber(u64),
    /// block timestamp
    Timestamp(u64),
    /// chain hardfork
    Hardfork(HardforkT),
}

/// Chain condition for selecting the right base fee params
pub struct BaseFeeCondition<HardforkT> {
    /// block current hardfork
    pub hardfork: Option<HardforkT>,
    /// block timestamp
    pub timestamp: Option<u64>,
    /// block number
    pub block_number: Option<u64>,
}
/// A mapping of hardfork to [`ConstantBaseFeeParams`]. This is used to specify
/// dynamic EIP-1559 parameters for chains like OP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariableBaseFeeParams<HardforkT: 'static> {
    activations: &'static [(DynamicBaseFeeCondition<HardforkT>, ConstantBaseFeeParams)],
}

impl<HardforkT: PartialOrd> VariableBaseFeeParams<HardforkT> {
    /// Constructs a new instance from the provided mapping.
    pub const fn new(
        activations: &'static [(DynamicBaseFeeCondition<HardforkT>, ConstantBaseFeeParams)],
    ) -> Self {
        Self { activations }
    }

    /// Selects the right [`ConstantBaseFeeParams`] for the given conditions, if any.
    pub fn at_condition(
        &self,
        condition: BaseFeeCondition<HardforkT>,
    ) -> Option<&ConstantBaseFeeParams> {
        self.activations
            .iter()
            .rev()
            .find(|(activation, _)| match activation {
                DynamicBaseFeeCondition::BlockNumber(activation_number) => condition.block_number.filter(|condition_number| *activation_number <= *condition_number).is_some(),
                DynamicBaseFeeCondition::Timestamp(activation_timestamp) => condition.timestamp.filter(|condition_timestamp| *activation_timestamp <= *condition_timestamp).is_some(),
                DynamicBaseFeeCondition::Hardfork(activation_hardfork) => condition.hardfork.as_ref().filter(|condition_hardfork| *activation_hardfork <= **condition_hardfork).is_some(),
            })
            .map(|(_, params)| params)
    }
}

/// Type that allows specifying constant or dynamic EIP-1559 parameters based on
/// the active hardfork.
pub enum BaseFeeParams<HardforkT: 'static> {
    /// Constant [`ConstantBaseFeeParams`]; used for chains that don't have
    /// dynamic EIP-1559 parameters
    Constant(ConstantBaseFeeParams),
    /// Variable [`ConstantBaseFeeParams`]; used for chains that have dynamic
    /// EIP-1559 parameters like OP
    Variable(VariableBaseFeeParams<HardforkT>),
}

impl<HardforkT: PartialOrd> BaseFeeParams<HardforkT> {
            
    /// Retrieves the right [`ConstantBaseFeeParams`] for the given conditions, if any.
    pub fn at_condition(
        &self,
        condition: BaseFeeCondition<HardforkT>,
    ) -> Option<&ConstantBaseFeeParams> {
        match self {
            Self::Constant(params) => Some(params),
            Self::Variable(params) => params.at_condition(condition),
        }
    }
}

impl<HardforkT> From<ConstantBaseFeeParams> for BaseFeeParams<HardforkT> {
    fn from(params: ConstantBaseFeeParams) -> Self {
        Self::Constant(params)
    }
}

impl<HardforkT> From<VariableBaseFeeParams<HardforkT>> for BaseFeeParams<HardforkT> {
    fn from(params: VariableBaseFeeParams<HardforkT>) -> Self {
        Self::Variable(params)
    }
}
