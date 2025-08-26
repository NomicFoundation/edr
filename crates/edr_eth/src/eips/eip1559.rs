pub use alloy_eips::eip1559::BaseFeeParams as ConstantBaseFeeParams;

/// Possible activation points of different base fee parameters
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, Hash, PartialEq, serde::Serialize)]
pub enum BaseFeeActivation<HardforkT> {
    /// block number
    BlockNumber(u64),
    /// chain hardfork
    Hardfork(HardforkT),
}
/// A mapping of hardfork to [`ConstantBaseFeeParams`]. This is used to specify
/// dynamic EIP-1559 parameters for chains like OP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariableBaseFeeParams<HardforkT> {
    activations: Vec<(BaseFeeActivation<HardforkT>, ConstantBaseFeeParams)>,
}

impl<HardforkT: PartialOrd> VariableBaseFeeParams<HardforkT> {
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
    Variable(VariableBaseFeeParams<HardforkT>),
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
            Self::Variable(params) => params.at_condition(hardfork, block_number),
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
