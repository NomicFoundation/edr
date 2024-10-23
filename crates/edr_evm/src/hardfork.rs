/// Ethereum L1 hardforks.
pub mod l1;

use derive_where::derive_where;

use crate::spec::RuntimeSpec;

/// Fork condition for a hardfork.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum ForkCondition {
    /// Activation based on block number.
    Block(u64),
    /// Activation based on UNIX timestamp.
    Timestamp(u64),
}

/// A struct that stores the hardforks for a chain.
#[derive_where(Clone, Debug; ChainSpecT::Hardfork)]
pub struct Activations<ChainSpecT: RuntimeSpec> {
    /// (Start block number -> `SpecId`) mapping
    hardforks: Vec<(ForkCondition, ChainSpecT::Hardfork)>,
}

impl<ChainSpecT: RuntimeSpec> Activations<ChainSpecT> {
    /// Constructs a new instance with the provided hardforks.
    pub fn new(hardforks: Vec<(ForkCondition, ChainSpecT::Hardfork)>) -> Self {
        Self { hardforks }
    }

    /// Creates a new instance for a new chain with the provided hardfork.
    pub fn with_spec_id(spec_id: ChainSpecT::Hardfork) -> Self {
        Self {
            hardforks: vec![(ForkCondition::Block(0), spec_id)],
        }
    }

    /// Whether no hardforks activations are present.
    pub fn is_empty(&self) -> bool {
        self.hardforks.is_empty()
    }

    /// Returns the hardfork's `SpecId` corresponding to the provided block
    /// number.
    pub fn hardfork_at_block(
        &self,
        block_number: u64,
        timestamp: u64,
    ) -> Option<ChainSpecT::Hardfork> {
        self.hardforks
            .iter()
            .rev()
            .find(|(criteria, _)| match criteria {
                ForkCondition::Block(activation) => block_number >= *activation,
                ForkCondition::Timestamp(activation) => timestamp >= *activation,
            })
            .map(|entry| entry.1)
    }

    /// Views the activations as for a different chain spec (that shares the
    /// underlying hardforks).
    pub fn as_chain_spec<OtherChainSpecT: RuntimeSpec<Hardfork = ChainSpecT::Hardfork>>(
        &'static self,
    ) -> &'static Activations<OtherChainSpecT> {
        // SAFETY: The layout is the same as we're using the same struct and the
        // Hardfork associated type is the same and we are also converting from
        // one static reference to another, so no lifetime hazards here as well.
        unsafe { std::mem::transmute(self) }
    }
}

impl<ChainSpecT: RuntimeSpec> From<&[(ForkCondition, ChainSpecT::Hardfork)]>
    for Activations<ChainSpecT>
{
    fn from(hardforks: &[(ForkCondition, ChainSpecT::Hardfork)]) -> Self {
        Self {
            hardforks: hardforks.to_vec(),
        }
    }
}

impl<'deserializer, ChainSpecT> serde::Deserialize<'deserializer> for Activations<ChainSpecT>
where
    ChainSpecT: RuntimeSpec<Hardfork: serde::Deserialize<'deserializer>>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'deserializer>,
    {
        let hardforks = Vec::<(ForkCondition, ChainSpecT::Hardfork)>::deserialize(deserializer)?;
        Ok(Self { hardforks })
    }
}

impl<ChainSpecT> serde::Serialize for Activations<ChainSpecT>
where
    ChainSpecT: RuntimeSpec<Hardfork: serde::Serialize>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.hardforks.serialize(serializer)
    }
}

/// Type that stores the configuration for a chain.
pub struct ChainConfig<ChainSpecT: RuntimeSpec> {
    /// Chain name
    pub name: String,
    /// Hardfork activations for the chain
    pub hardfork_activations: Activations<ChainSpecT>,
}
