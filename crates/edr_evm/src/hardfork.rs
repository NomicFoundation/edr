/// Ethereum L1 hardforks.
pub mod l1;

/// Fork condition for a hardfork.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum ForkCondition {
    /// Activation based on block number.
    Block(u64),
    /// Activation based on UNIX timestamp.
    Timestamp(u64),
}

/// A struct that stores the hardforks for a chain.
#[derive(Clone, Debug)]
pub struct Activations<HardforkT> {
    /// (Start block number -> hardfork) mapping
    hardforks: Vec<(ForkCondition, HardforkT)>,
}

impl<HardforkT> Activations<HardforkT> {
    /// Constructs a new instance with the provided hardforks.
    pub fn new(hardforks: Vec<(ForkCondition, HardforkT)>) -> Self {
        Self { hardforks }
    }

    /// Creates a new instance for a new chain with the provided hardfork.
    pub fn with_spec_id(hardfork: HardforkT) -> Self {
        Self {
            hardforks: vec![(ForkCondition::Block(0), hardfork)],
        }
    }

    /// Whether no hardforks activations are present.
    pub fn is_empty(&self) -> bool {
        self.hardforks.is_empty()
    }
}

impl<HardforkT: Clone> Activations<HardforkT> {
    /// Returns the hardfork's `SpecId` corresponding to the provided block
    /// number.
    pub fn hardfork_at_block(&self, block_number: u64, timestamp: u64) -> Option<HardforkT> {
        self.hardforks
            .iter()
            .rev()
            .find(|(criteria, _)| match criteria {
                ForkCondition::Block(activation) => block_number >= *activation,
                ForkCondition::Timestamp(activation) => timestamp >= *activation,
            })
            .map(|entry| entry.1.clone())
    }
}

impl<HardforkT: Clone> From<&[(ForkCondition, HardforkT)]> for Activations<HardforkT> {
    fn from(hardforks: &[(ForkCondition, HardforkT)]) -> Self {
        Self {
            hardforks: hardforks.to_vec(),
        }
    }
}

impl<'deserializer, HardforkT> serde::Deserialize<'deserializer> for Activations<HardforkT>
where
    HardforkT: serde::Deserialize<'deserializer>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'deserializer>,
    {
        let hardforks = Vec::<(ForkCondition, HardforkT)>::deserialize(deserializer)?;
        Ok(Self { hardforks })
    }
}

impl<HardforkT> serde::Serialize for Activations<HardforkT>
where
    HardforkT: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.hardforks.serialize(serializer)
    }
}

/// Type that stores the configuration for a chain.
pub struct ChainConfig<HardforkT> {
    /// Chain name
    pub name: String,
    /// Hardfork activations for the chain
    pub hardfork_activations: Activations<HardforkT>,
}
