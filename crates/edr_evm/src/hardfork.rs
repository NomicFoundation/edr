/// Ethereum L1 hardforks.
pub mod l1;

/// Fork condition for a hardfork.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ForkCondition {
    /// Activation based on block number.
    Block(u64),
    /// Activation based on UNIX timestamp.
    Timestamp(u64),
}

/// A type representing the activation of a hardfork.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activation<HardforkT> {
    /// The condition for the hardfork activation.
    pub condition: ForkCondition,
    /// The hardfork to be activated.
    pub hardfork: HardforkT,
}

/// A struct that stores the hardforks for a chain.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct Activations<HardforkT> {
    /// (Start block number -> hardfork) mapping
    hardforks: Vec<Activation<HardforkT>>,
}

impl<HardforkT> Activations<HardforkT> {
    /// Constructs a new instance with the provided hardforks.
    pub fn new(hardforks: Vec<Activation<HardforkT>>) -> Self {
        Self { hardforks }
    }

    /// Returns the inner hardforks.
    pub fn into_inner(self) -> Vec<Activation<HardforkT>> {
        self.hardforks
    }

    /// Creates a new instance for a new chain with the provided hardfork.
    pub fn with_spec_id(hardfork: HardforkT) -> Self {
        Self {
            hardforks: vec![Activation {
                condition: ForkCondition::Block(0),
                hardfork,
            }],
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
            .find(|Activation { condition, .. }| match condition {
                ForkCondition::Block(activation) => block_number >= *activation,
                ForkCondition::Timestamp(activation) => timestamp >= *activation,
            })
            .map(|activation| activation.hardfork.clone())
    }
}

impl<HardforkT: Clone> From<&[Activation<HardforkT>]> for Activations<HardforkT> {
    fn from(hardforks: &[Activation<HardforkT>]) -> Self {
        Self {
            hardforks: hardforks.to_vec(),
        }
    }
}

/// Type that stores the configuration for a chain.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainConfig<HardforkT> {
    /// Chain name
    pub name: String,
    /// Hardfork activations for the chain
    pub hardfork_activations: Activations<HardforkT>,
}
