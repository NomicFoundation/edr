use std::sync::LazyLock;

use edr_chain_config::ChainConfig;
use edr_chain_spec::EvmSpecId;
use edr_eip1559::BaseFeeParams;
use edr_primitives::{HashMap, UnknownHardfork};

use crate::Hardfork;

/// Base chain configs
pub mod base;
pub mod generated;
/// OP chain configs
pub mod op;

/// OP Stack hardfork.
///
/// Models protocol upgrades, including ones without EVM-semantics changes,
/// unlike [`op_revm::OpSpecId`] which models EVM behavior classes.
///
/// The `strum(serialize = …)` strings must stay identical to the [`name`]
/// module constants.
#[repr(u8)]
#[allow(non_camel_case_types)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
)]
#[strum(parse_err_ty = UnknownHardfork, parse_err_fn = unknown_hardfork)]
pub enum OpHardfork {
    /// Bedrock hardfork
    #[strum(serialize = "Bedrock")]
    Bedrock = 100,
    /// Regolith hardfork
    #[strum(serialize = "Regolith")]
    Regolith,
    /// Canyon hardfork
    #[strum(serialize = "Canyon")]
    Canyon,
    /// Ecotone hardfork
    #[strum(serialize = "Ecotone")]
    Ecotone,
    /// Fjord hardfork
    #[strum(serialize = "Fjord")]
    Fjord,
    /// Granite hardfork
    #[strum(serialize = "Granite")]
    Granite,
    /// Holocene hardfork
    #[strum(serialize = "Holocene")]
    Holocene,
    /// Isthmus hardfork
    #[strum(serialize = "Isthmus")]
    Isthmus,
    /// Jovian hardfork
    #[default]
    #[strum(serialize = "Jovian")]
    Jovian,
    /// Interop hardfork
    #[strum(serialize = "Interop")]
    Interop,
}

fn unknown_hardfork(_name: &str) -> UnknownHardfork {
    UnknownHardfork
}

impl From<OpHardfork> for op_revm::OpSpecId {
    fn from(hardfork: OpHardfork) -> Self {
        match hardfork {
            OpHardfork::Bedrock => op_revm::OpSpecId::BEDROCK,
            OpHardfork::Regolith => op_revm::OpSpecId::REGOLITH,
            OpHardfork::Canyon => op_revm::OpSpecId::CANYON,
            OpHardfork::Ecotone => op_revm::OpSpecId::ECOTONE,
            OpHardfork::Fjord => op_revm::OpSpecId::FJORD,
            OpHardfork::Granite => op_revm::OpSpecId::GRANITE,
            OpHardfork::Holocene => op_revm::OpSpecId::HOLOCENE,
            OpHardfork::Isthmus => op_revm::OpSpecId::ISTHMUS,
            OpHardfork::Jovian => op_revm::OpSpecId::JOVIAN,
            OpHardfork::Interop => op_revm::OpSpecId::INTEROP,
        }
    }
}

impl From<OpHardfork> for EvmSpecId {
    fn from(hardfork: OpHardfork) -> Self {
        op_revm::OpSpecId::from(hardfork).into_eth_spec()
    }
}

/// String identifiers for OP hardforks.
pub mod name {
    /// String identifier for the Bedrock hardfork
    pub const BEDROCK: &str = "Bedrock";
    /// String identifier for the Regolith hardfork
    pub const REGOLITH: &str = "Regolith";
    /// String identifier for the Canyon hardfork
    pub const CANYON: &str = "Canyon";
    /// String identifier for the Ecotone hardfork
    pub const ECOTONE: &str = "Ecotone";
    /// String identifier for the Fjord hardfork
    pub const FJORD: &str = "Fjord";
    /// String identifier for the Granite hardfork
    pub const GRANITE: &str = "Granite";
    /// String identifier for the Holocene hardfork
    pub const HOLOCENE: &str = "Holocene";
    /// String identifier for the Isthmus hardfork
    pub const ISTHMUS: &str = "Isthmus";
    /// String identifier for the Jovian hardfork
    pub const JOVIAN: &str = "Jovian";
    /// String identifier for the Interop hardfork
    pub const INTEROP: &str = "Interop";
}

/// Returns the chain configurations for OP chains.
pub fn op_chain_configs() -> &'static HashMap<u64, ChainConfig<Hardfork>> {
    static CONFIGS: LazyLock<HashMap<u64, ChainConfig<Hardfork>>> = LazyLock::new(|| {
        let mut configs = generated::chain_configs();

        // Override `base_fee_params` for `op` blockchains
        // TODO: remove this override once https://github.com/NomicFoundation/edr/issues/1072 is implemented
        configs
            .entry(op::MAINNET_CHAIN_ID)
            .and_modify(|entry| entry.base_fee_params = op::MAINNET_BASE_FEE_PARAMS.clone());
        configs
            .entry(op::SEPOLIA_CHAIN_ID)
            .and_modify(|entry| entry.base_fee_params = op::SEPOLIA_BASE_FEE_PARAMS.clone());

        // Override `base_fee_params` for `base` blockchains
        // TODO: remove this override once https://github.com/NomicFoundation/edr/issues/1072 is implemented
        configs
            .entry(base::MAINNET_CHAIN_ID)
            .and_modify(|entry| entry.base_fee_params = base::MAINNET_BASE_FEE_PARAMS.clone());
        configs
            .entry(base::SEPOLIA_CHAIN_ID)
            .and_modify(|entry| entry.base_fee_params = base::SEPOLIA_BASE_FEE_PARAMS.clone());
        configs
    });

    &CONFIGS
}

/// Returns the corresponding configuration for the provided chain ID, if
/// it is supported.
pub fn op_chain_config(chain_id: u64) -> Option<&'static ChainConfig<Hardfork>> {
    op_chain_configs().get(&chain_id)
}

/// Returns the default base fee params to fallback to
pub fn op_default_base_fee_params() -> &'static BaseFeeParams<Hardfork> {
    &op::MAINNET_BASE_FEE_PARAMS
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    const VARIANTS: [OpHardfork; 10] = [
        OpHardfork::Bedrock,
        OpHardfork::Regolith,
        OpHardfork::Canyon,
        OpHardfork::Ecotone,
        OpHardfork::Fjord,
        OpHardfork::Granite,
        OpHardfork::Holocene,
        OpHardfork::Isthmus,
        OpHardfork::Jovian,
        OpHardfork::Interop,
    ];

    #[test]
    fn ordering_matches_activation_order() {
        for window in VARIANTS.windows(2) {
            assert!(window[0] < window[1]);
        }
    }

    /// The strings the `strum` derives emit/parse must stay in sync with the
    /// [`name`] module constants, which are re-exported as public API.
    const NAMES: [&str; 10] = [
        name::BEDROCK,
        name::REGOLITH,
        name::CANYON,
        name::ECOTONE,
        name::FJORD,
        name::GRANITE,
        name::HOLOCENE,
        name::ISTHMUS,
        name::JOVIAN,
        name::INTEROP,
    ];

    #[test]
    fn name_round_trip() {
        for (hardfork, name) in VARIANTS.into_iter().zip(NAMES) {
            assert_eq!(hardfork.to_string(), name);
            assert_eq!(<&'static str>::from(hardfork), name);
            assert_eq!(OpHardfork::from_str(name), Ok(hardfork));
        }

        assert_eq!(OpHardfork::from_str("NotAHardfork"), Err(UnknownHardfork));
        // strum must not fall back to parsing variant identifiers.
        assert_eq!(OpHardfork::from_str("BEDROCK"), Err(UnknownHardfork));
    }

    #[test]
    fn serde_round_trip() {
        for hardfork in VARIANTS {
            let json = serde_json::to_string(&hardfork).expect("serialization succeeds");
            let roundtrip: OpHardfork =
                serde_json::from_str(&json).expect("deserialization succeeds");
            assert_eq!(roundtrip, hardfork);
        }
    }

    #[test]
    fn default_hardfork() {
        assert_eq!(OpHardfork::default(), OpHardfork::Jovian);
    }

    /// `OpChainSpec::default_block_difficulty` reports zero for every hardfork,
    /// and `OpBlockBuilder` pays no block reward. Both are only correct because
    /// none of these hardforks precede the merge.
    #[test]
    fn every_hardfork_is_post_merge() {
        for hardfork in VARIANTS {
            assert!(EvmSpecId::from(hardfork) >= EvmSpecId::MERGE, "{hardfork}");
        }
    }

    #[test]
    fn defaults() {
        assert_eq!(
            op_revm::OpSpecId::from(OpHardfork::default()),
            op_revm::OpSpecId::default()
        );
    }
}
