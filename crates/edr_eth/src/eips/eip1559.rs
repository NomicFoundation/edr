pub use alloy_eips::eip1559::BaseFeeParams as ConstantBaseFeeParams;
use derive_where::derive_where;
use revm_primitives::EvmWiring;

/// A mapping of hardfork to [`ConstantBaseFeeParams`]. This is used to specify
/// dynamic EIP-1559 parameters for chains like Optimism.
#[derive_where(Clone, Debug, PartialEq, Eq; ChainSpecT::Hardfork)]
pub struct ForkBaseFeeParams<ChainSpecT: EvmWiring> {
    activations: &'static [(ChainSpecT::Hardfork, ConstantBaseFeeParams)],
}

impl<ChainSpecT: EvmWiring> ForkBaseFeeParams<ChainSpecT> {
    /// Constructs a new instance from the provided mapping.
    pub const fn new(
        activations: &'static [(ChainSpecT::Hardfork, ConstantBaseFeeParams)],
    ) -> Self {
        Self { activations }
    }
}

/// Type that allows specifying constant or dynamic EIP-1559 parameters based on
/// the active hardfork.
pub enum BaseFeeParams<ChainSpecT: EvmWiring> {
    /// Constant [`ConstantBaseFeeParams`]; used for chains that don't have
    /// dynamic EIP-1559 parameters
    Constant(ConstantBaseFeeParams),
    /// Variable [`ConstantBaseFeeParams`]; used for chains that have dynamic
    /// EIP-1559 parameters like Optimism
    Variable(ForkBaseFeeParams<ChainSpecT>),
}

impl<ChainSpecT: EvmWiring<Hardfork: PartialOrd>> BaseFeeParams<ChainSpecT> {
    /// Retrieves the [`ConstantBaseFeeParams`] for the given hardfork, if any.
    pub fn at_hardfork(&self, hardfork: ChainSpecT::Hardfork) -> Option<&ConstantBaseFeeParams> {
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

impl<ChainSpecT: EvmWiring> From<ConstantBaseFeeParams> for BaseFeeParams<ChainSpecT> {
    fn from(params: ConstantBaseFeeParams) -> Self {
        Self::Constant(params)
    }
}

impl<ChainSpecT: EvmWiring> From<ForkBaseFeeParams<ChainSpecT>> for BaseFeeParams<ChainSpecT> {
    fn from(params: ForkBaseFeeParams<ChainSpecT>) -> Self {
        Self::Variable(params)
    }
}
