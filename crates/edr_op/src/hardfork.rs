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
    BEDROCK = 100,
    /// Regolith hardfork
    #[strum(serialize = "Regolith")]
    REGOLITH,
    /// Canyon hardfork
    #[strum(serialize = "Canyon")]
    CANYON,
    /// Ecotone hardfork
    #[strum(serialize = "Ecotone")]
    ECOTONE,
    /// Fjord hardfork
    #[strum(serialize = "Fjord")]
    FJORD,
    /// Granite hardfork
    #[strum(serialize = "Granite")]
    GRANITE,
    /// Holocene hardfork
    #[strum(serialize = "Holocene")]
    HOLOCENE,
    /// Isthmus hardfork
    #[strum(serialize = "Isthmus")]
    ISTHMUS,
    /// Jovian hardfork
    #[default]
    #[strum(serialize = "Jovian")]
    JOVIAN,
    /// Interop hardfork
    #[strum(serialize = "Interop")]
    INTEROP,
    /// Osaka hardfork
    #[strum(serialize = "Osaka")]
    OSAKA,
}

fn unknown_hardfork(_name: &str) -> UnknownHardfork {
    UnknownHardfork
}

impl From<OpHardfork> for op_revm::OpSpecId {
    fn from(hardfork: OpHardfork) -> Self {
        match hardfork {
            OpHardfork::BEDROCK => op_revm::OpSpecId::BEDROCK,
            OpHardfork::REGOLITH => op_revm::OpSpecId::REGOLITH,
            OpHardfork::CANYON => op_revm::OpSpecId::CANYON,
            OpHardfork::ECOTONE => op_revm::OpSpecId::ECOTONE,
            OpHardfork::FJORD => op_revm::OpSpecId::FJORD,
            OpHardfork::GRANITE => op_revm::OpSpecId::GRANITE,
            OpHardfork::HOLOCENE => op_revm::OpSpecId::HOLOCENE,
            OpHardfork::ISTHMUS => op_revm::OpSpecId::ISTHMUS,
            OpHardfork::JOVIAN => op_revm::OpSpecId::JOVIAN,
            OpHardfork::INTEROP => op_revm::OpSpecId::INTEROP,
            OpHardfork::OSAKA => op_revm::OpSpecId::OSAKA,
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
    /// String identifier for the Osaka hardfork, borrowed from L1.
    pub const OSAKA: &str = "Osaka";
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

    const VARIANTS: [OpHardfork; 11] = [
        OpHardfork::BEDROCK,
        OpHardfork::REGOLITH,
        OpHardfork::CANYON,
        OpHardfork::ECOTONE,
        OpHardfork::FJORD,
        OpHardfork::GRANITE,
        OpHardfork::HOLOCENE,
        OpHardfork::ISTHMUS,
        OpHardfork::JOVIAN,
        OpHardfork::INTEROP,
        OpHardfork::OSAKA,
    ];

    #[test]
    fn ordering_matches_activation_order() {
        for window in VARIANTS.windows(2) {
            assert!(window[0] < window[1]);
        }
    }

    /// The strings the `strum` derives emit/parse must stay in sync with the
    /// [`name`] module constants, which are re-exported as public API.
    const NAMES: [&str; 11] = [
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
        name::OSAKA,
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
        assert_eq!(OpHardfork::default(), OpHardfork::JOVIAN);
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

    /// Parity tests against op-revm's `OpSpecId`, which still models every OP
    /// protocol upgrade at the currently pinned op-revm version. They
    /// guarantee that the owned enum is a lossless 1:1 mirror of op-revm
    /// during the transition; DELETE THIS MODULE when an op-revm upgrade
    /// removes variants from `OpSpecId`.
    mod revm_parity {
        use op_revm::OpSpecId;

        use super::*;

        #[test]
        fn discriminants() {
            for hardfork in VARIANTS {
                assert_eq!(hardfork as u8, OpSpecId::from(hardfork) as u8, "{hardfork}");
            }
        }

        #[test]
        fn eth_spec_mapping() {
            for hardfork in VARIANTS {
                assert_eq!(
                    EvmSpecId::from(hardfork),
                    OpSpecId::from(hardfork).into_eth_spec(),
                    "{hardfork}"
                );
            }
        }

        #[test]
        fn names_and_from_str() {
            for hardfork in VARIANTS {
                let name: &'static str = hardfork.into();
                let op_name: &'static str = OpSpecId::from(hardfork).into();
                assert_eq!(name, op_name);
                assert_eq!(OpSpecId::from_str(name), Ok(OpSpecId::from(hardfork)));
            }
        }

        #[test]
        fn serde_tokens() {
            for hardfork in VARIANTS {
                let json = serde_json::to_string(&hardfork).expect("serialization succeeds");
                let op_json = serde_json::to_string(&OpSpecId::from(hardfork))
                    .expect("serialization succeeds");
                assert_eq!(json, op_json);
            }
        }

        #[test]
        fn defaults() {
            assert_eq!(OpSpecId::from(OpHardfork::default()), OpSpecId::default());
        }
    }
}
