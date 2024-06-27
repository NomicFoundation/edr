/// Ethereum L1 hardforks.
pub mod l1;

use derive_where::derive_where;

use crate::chain_spec::ChainSpec;

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
pub struct Activations<ChainSpecT: ChainSpec> {
    /// (Start block number -> `SpecId`) mapping
    hardforks: Vec<(ForkCondition, ChainSpecT::Hardfork)>,
}

impl<ChainSpecT: ChainSpec> Activations<ChainSpecT> {
    /// Constructs a new instance with the provided hardforks.
    pub fn new(hardforks: Vec<(ForkCondition, ChainSpecT::Hardfork)>) -> Self {
        Self { hardforks }
    }

    /// Creates a new instance for a new chain with the provided [`SpecId`].
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

    /// Retrieves the activation criteria at which the provided hardfork was
    /// activated.
    pub fn hardfork_activation(&self, spec_id: ChainSpecT::Hardfork) -> Option<ForkCondition> {
        self.hardforks
            .iter()
            .find(|(_, id)| *id == spec_id)
            .map(|(criteria, _)| criteria.clone())
    }
}

impl<ChainSpecT: ChainSpec> From<&[(ForkCondition, ChainSpecT::Hardfork)]>
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
    ChainSpecT: ChainSpec<Hardfork: serde::Deserialize<'deserializer>>,
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
    ChainSpecT: ChainSpec<Hardfork: serde::Serialize>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.hardforks.serialize(serializer)
    }
}

/// Type that stores the configuration for a chain.
pub struct ChainConfig<ChainSpecT: ChainSpec> {
    /// Chain name
    pub name: String,
    /// Hardfork activations for the chain
    pub hardfork_activations: Activations<ChainSpecT>,
}
