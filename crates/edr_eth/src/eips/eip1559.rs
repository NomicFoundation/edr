pub use alloy_eips::eip1559::BaseFeeParams as ConstantBaseFeeParams;

/// A mapping of hardfork to [`ConstantBaseFeeParams`]. This is used to specify
/// dynamic EIP-1559 parameters for chains like OP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkBaseFeeParams<HardforkT: 'static> {
    activations: &'static [(HardforkT, ConstantBaseFeeParams)],
}

impl<HardforkT> ForkBaseFeeParams<HardforkT> {
    /// Constructs a new instance from the provided mapping.
    pub const fn new(activations: &'static [(HardforkT, ConstantBaseFeeParams)]) -> Self {
        Self { activations }
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
    Variable(ForkBaseFeeParams<HardforkT>),
}

impl<HardforkT: PartialOrd> BaseFeeParams<HardforkT> {
    /// Retrieves the [`ConstantBaseFeeParams`] for the given hardfork, if any.
    pub fn at_hardfork(&self, hardfork: HardforkT) -> Option<&ConstantBaseFeeParams> {
        match self {
            Self::Constant(params) => Some(params),
            Self::Variable(params) => params
                .activations
                .iter()
                .rev()
                .find(|(activation, _)| *activation <= hardfork)
                .map(|(_, params)| params),
        }
    }
}

impl<HardforkT> From<ConstantBaseFeeParams> for BaseFeeParams<HardforkT> {
    fn from(params: ConstantBaseFeeParams) -> Self {
        Self::Constant(params)
    }
}

impl<HardforkT> From<ForkBaseFeeParams<HardforkT>> for BaseFeeParams<HardforkT> {
    fn from(params: ForkBaseFeeParams<HardforkT>) -> Self {
        Self::Variable(params)
    }
}
