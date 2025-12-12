use edr_eip1559::BaseFeeParams;
use edr_eip7892::ScheduledBlobParams;

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
pub struct HardforkActivation<HardforkT> {
    /// The condition for the hardfork activation.
    pub condition: ForkCondition,
    /// The hardfork to be activated.
    pub hardfork: HardforkT,
}

/// A struct that stores the hardforks for a chain.
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct HardforkActivations<HardforkT> {
    /// (Start block number -> hardfork) mapping
    hardforks: Vec<HardforkActivation<HardforkT>>,
}

impl<HardforkT> HardforkActivations<HardforkT> {
    /// Constructs a new instance with the provided hardforks.
    pub fn new(hardforks: Vec<HardforkActivation<HardforkT>>) -> Self {
        Self { hardforks }
    }

    /// Returns the inner hardforks.
    pub fn into_inner(self) -> Vec<HardforkActivation<HardforkT>> {
        self.hardforks
    }

    /// Creates a new instance for a new chain with the provided hardfork.
    pub fn with_spec_id(hardfork: HardforkT) -> Self {
        Self {
            hardforks: vec![HardforkActivation {
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

impl<HardforkT: Clone> HardforkActivations<HardforkT> {
    /// Returns the hardfork's `SpecId` corresponding to the provided block
    /// number.
    pub fn hardfork_at_block(&self, block_number: u64, timestamp: u64) -> Option<HardforkT> {
        self.hardforks
            .iter()
            .rev()
            .find(|HardforkActivation { condition, .. }| match condition {
                ForkCondition::Block(activation) => block_number >= *activation,
                ForkCondition::Timestamp(activation) => timestamp >= *activation,
            })
            .map(|activation| activation.hardfork.clone())
    }
}

impl<HardforkT: Clone> From<&[HardforkActivation<HardforkT>]> for HardforkActivations<HardforkT> {
    fn from(hardforks: &[HardforkActivation<HardforkT>]) -> Self {
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
    pub hardfork_activations: HardforkActivations<HardforkT>,
    /// Base fee param activations for the chain
    pub base_fee_params: BaseFeeParams<HardforkT>,
    /// Blob Parameter Only hardforks schedule
    pub bpo_hardfork_schedule: Option<ScheduledBlobParams>,
}

impl<HardforkT: Clone> ChainConfig<HardforkT> {
    /// Applies the provided override to the current instance, while keeping the
    /// name the same.
    pub fn apply_override(&mut self, override_config: &ChainOverride<HardforkT>) {
        if let Some(hardfork_activations) = &override_config.hardfork_activation_overrides {
            self.hardfork_activations = hardfork_activations.clone();
        }
    }
}

/// Type that stores the configuration for a chain.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainOverride<HardforkT> {
    /// Chain name
    pub name: String,
    /// Hardfork activations for the chain
    pub hardfork_activation_overrides: Option<HardforkActivations<HardforkT>>,
}
