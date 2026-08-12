//! Ethereum L1 hardfork type, owned by EDR.

use edr_chain_spec::EvmSpecId;
use edr_primitives::UnknownHardfork;

/// Ethereum L1 hardfork.
///
/// Models protocol upgrades, including ones without EVM-semantics changes,
/// unlike [`EvmSpecId`] which models EVM behavior classes.
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
pub enum L1Hardfork {
    /// Frontier hardfork
    #[strum(serialize = "Frontier")]
    FRONTIER = 0,
    /// Frontier Thawing hardfork
    #[strum(serialize = "Frontier Thawing")]
    FRONTIER_THAWING,
    /// Homestead hardfork
    #[strum(serialize = "Homestead")]
    HOMESTEAD,
    /// DAO Fork hardfork
    #[strum(serialize = "DAO Fork")]
    DAO_FORK,
    /// Tangerine Whistle hardfork
    #[strum(serialize = "Tangerine")]
    TANGERINE,
    /// Spurious Dragon hardfork
    #[strum(serialize = "Spurious")]
    SPURIOUS_DRAGON,
    /// Byzantium hardfork
    #[strum(serialize = "Byzantium")]
    BYZANTIUM,
    /// Constantinople hardfork
    #[strum(serialize = "Constantinople")]
    CONSTANTINOPLE,
    /// Petersburg hardfork
    #[strum(serialize = "Petersburg")]
    PETERSBURG,
    /// Istanbul hardfork
    #[strum(serialize = "Istanbul")]
    ISTANBUL,
    /// Muir Glacier hardfork
    #[strum(serialize = "MuirGlacier")]
    MUIR_GLACIER,
    /// Berlin hardfork
    #[strum(serialize = "Berlin")]
    BERLIN,
    /// London hardfork
    #[strum(serialize = "London")]
    LONDON,
    /// Arrow Glacier hardfork
    #[strum(serialize = "Arrow Glacier")]
    ARROW_GLACIER,
    /// Gray Glacier hardfork
    #[strum(serialize = "Gray Glacier")]
    GRAY_GLACIER,
    /// Paris/Merge hardfork
    #[strum(serialize = "Merge")]
    MERGE,
    /// Shanghai hardfork
    #[strum(serialize = "Shanghai")]
    SHANGHAI,
    /// Cancun hardfork
    #[strum(serialize = "Cancun")]
    CANCUN,
    /// Prague hardfork
    #[strum(serialize = "Prague")]
    PRAGUE,
    /// Osaka hardfork
    #[default]
    #[strum(serialize = "Osaka")]
    OSAKA,
    /// Amsterdam hardfork
    #[strum(serialize = "Amsterdam")]
    AMSTERDAM,
}

fn unknown_hardfork(_name: &str) -> UnknownHardfork {
    UnknownHardfork
}

impl From<L1Hardfork> for EvmSpecId {
    fn from(hardfork: L1Hardfork) -> Self {
        match hardfork {
            L1Hardfork::FRONTIER => EvmSpecId::FRONTIER,
            L1Hardfork::FRONTIER_THAWING => EvmSpecId::FRONTIER_THAWING,
            L1Hardfork::HOMESTEAD => EvmSpecId::HOMESTEAD,
            L1Hardfork::DAO_FORK => EvmSpecId::DAO_FORK,
            L1Hardfork::TANGERINE => EvmSpecId::TANGERINE,
            L1Hardfork::SPURIOUS_DRAGON => EvmSpecId::SPURIOUS_DRAGON,
            L1Hardfork::BYZANTIUM => EvmSpecId::BYZANTIUM,
            L1Hardfork::CONSTANTINOPLE => EvmSpecId::CONSTANTINOPLE,
            L1Hardfork::PETERSBURG => EvmSpecId::PETERSBURG,
            L1Hardfork::ISTANBUL => EvmSpecId::ISTANBUL,
            L1Hardfork::MUIR_GLACIER => EvmSpecId::MUIR_GLACIER,
            L1Hardfork::BERLIN => EvmSpecId::BERLIN,
            L1Hardfork::LONDON => EvmSpecId::LONDON,
            L1Hardfork::ARROW_GLACIER => EvmSpecId::ARROW_GLACIER,
            L1Hardfork::GRAY_GLACIER => EvmSpecId::GRAY_GLACIER,
            L1Hardfork::MERGE => EvmSpecId::MERGE,
            L1Hardfork::SHANGHAI => EvmSpecId::SHANGHAI,
            L1Hardfork::CANCUN => EvmSpecId::CANCUN,
            L1Hardfork::PRAGUE => EvmSpecId::PRAGUE,
            L1Hardfork::OSAKA => EvmSpecId::OSAKA,
            L1Hardfork::AMSTERDAM => EvmSpecId::AMSTERDAM,
        }
    }
}

/// String identifiers for L1 hardforks.
pub mod name {
    /// String identifier for the Frontier hardfork
    pub const FRONTIER: &str = "Frontier";
    /// String identifier for the Frontier Thawing hardfork
    pub const FRONTIER_THAWING: &str = "Frontier Thawing";
    /// String identifier for the Homestead hardfork
    pub const HOMESTEAD: &str = "Homestead";
    /// String identifier for the DAO Fork hardfork
    pub const DAO_FORK: &str = "DAO Fork";
    /// String identifier for the Tangerine Whistle hardfork
    pub const TANGERINE: &str = "Tangerine";
    /// String identifier for the Spurious Dragon hardfork
    pub const SPURIOUS_DRAGON: &str = "Spurious";
    /// String identifier for the Byzantium hardfork
    pub const BYZANTIUM: &str = "Byzantium";
    /// String identifier for the Constantinople hardfork
    pub const CONSTANTINOPLE: &str = "Constantinople";
    /// String identifier for the Petersburg hardfork
    pub const PETERSBURG: &str = "Petersburg";
    /// String identifier for the Istanbul hardfork
    pub const ISTANBUL: &str = "Istanbul";
    /// String identifier for the Muir Glacier hardfork
    pub const MUIR_GLACIER: &str = "MuirGlacier";
    /// String identifier for the Berlin hardfork
    pub const BERLIN: &str = "Berlin";
    /// String identifier for the London hardfork
    pub const LONDON: &str = "London";
    /// String identifier for the Arrow Glacier hardfork
    pub const ARROW_GLACIER: &str = "Arrow Glacier";
    /// String identifier for the Gray Glacier hardfork
    pub const GRAY_GLACIER: &str = "Gray Glacier";
    /// String identifier for the Paris/Merge hardfork
    pub const MERGE: &str = "Merge";
    /// String identifier for the Shanghai hardfork
    pub const SHANGHAI: &str = "Shanghai";
    /// String identifier for the Cancun hardfork
    pub const CANCUN: &str = "Cancun";
    /// String identifier for the Prague hardfork
    pub const PRAGUE: &str = "Prague";
    /// String identifier for the Osaka hardfork
    pub const OSAKA: &str = "Osaka";
    /// String identifier for the Amsterdam hardfork
    pub const AMSTERDAM: &str = "Amsterdam";
    /// String identifier for the latest hardfork
    pub const LATEST: &str = "Latest";
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use super::*;

    const VARIANTS: [L1Hardfork; 21] = [
        L1Hardfork::FRONTIER,
        L1Hardfork::FRONTIER_THAWING,
        L1Hardfork::HOMESTEAD,
        L1Hardfork::DAO_FORK,
        L1Hardfork::TANGERINE,
        L1Hardfork::SPURIOUS_DRAGON,
        L1Hardfork::BYZANTIUM,
        L1Hardfork::CONSTANTINOPLE,
        L1Hardfork::PETERSBURG,
        L1Hardfork::ISTANBUL,
        L1Hardfork::MUIR_GLACIER,
        L1Hardfork::BERLIN,
        L1Hardfork::LONDON,
        L1Hardfork::ARROW_GLACIER,
        L1Hardfork::GRAY_GLACIER,
        L1Hardfork::MERGE,
        L1Hardfork::SHANGHAI,
        L1Hardfork::CANCUN,
        L1Hardfork::PRAGUE,
        L1Hardfork::OSAKA,
        L1Hardfork::AMSTERDAM,
    ];

    #[test]
    fn ordering_matches_activation_order() {
        for window in VARIANTS.windows(2) {
            assert!(window[0] < window[1]);
        }
    }

    /// The strings the `strum` derives emit/parse must stay in sync with the
    /// [`name`] module constants, which are re-exported as public API.
    const NAMES: [&str; 21] = [
        name::FRONTIER,
        name::FRONTIER_THAWING,
        name::HOMESTEAD,
        name::DAO_FORK,
        name::TANGERINE,
        name::SPURIOUS_DRAGON,
        name::BYZANTIUM,
        name::CONSTANTINOPLE,
        name::PETERSBURG,
        name::ISTANBUL,
        name::MUIR_GLACIER,
        name::BERLIN,
        name::LONDON,
        name::ARROW_GLACIER,
        name::GRAY_GLACIER,
        name::MERGE,
        name::SHANGHAI,
        name::CANCUN,
        name::PRAGUE,
        name::OSAKA,
        name::AMSTERDAM,
    ];

    #[test]
    fn name_round_trip() {
        for (hardfork, name) in VARIANTS.into_iter().zip(NAMES) {
            assert_eq!(hardfork.to_string(), name);
            assert_eq!(<&'static str>::from(hardfork), name);
            assert_eq!(L1Hardfork::from_str(name), Ok(hardfork));
        }

        assert_eq!(L1Hardfork::from_str("Latest"), Err(UnknownHardfork));
        assert_eq!(L1Hardfork::from_str("NotAHardfork"), Err(UnknownHardfork));
        // strum must not fall back to parsing variant identifiers.
        assert_eq!(L1Hardfork::from_str("MUIR_GLACIER"), Err(UnknownHardfork));
    }

    #[test]
    fn serde_round_trip() {
        for hardfork in VARIANTS {
            let json = serde_json::to_string(&hardfork).expect("serialization succeeds");
            let roundtrip: L1Hardfork =
                serde_json::from_str(&json).expect("deserialization succeeds");
            assert_eq!(roundtrip, hardfork);
        }
    }

    #[test]
    fn default_hardfork() {
        assert_eq!(L1Hardfork::default(), L1Hardfork::OSAKA);
    }

    /// Parity tests against revm's `SpecId`, which still models every L1
    /// protocol upgrade at the currently pinned revm version. They guarantee
    /// that the owned enum is a lossless 1:1 mirror of revm during the
    /// transition; DELETE THIS MODULE when a revm upgrade removes variants
    /// from `SpecId`.
    mod revm_parity {
        use super::*;

        #[test]
        fn discriminants() {
            for hardfork in VARIANTS {
                assert_eq!(
                    hardfork as u8,
                    EvmSpecId::from(hardfork) as u8,
                    "{hardfork}"
                );
            }
        }

        #[test]
        fn names_and_from_str() {
            for hardfork in VARIANTS {
                let name: &'static str = hardfork.into();
                let evm_name: &'static str = EvmSpecId::from(hardfork).into();
                assert_eq!(name, evm_name);
                assert_eq!(hardfork.to_string(), EvmSpecId::from(hardfork).to_string());
                assert_eq!(EvmSpecId::from_str(name), Ok(EvmSpecId::from(hardfork)));
            }
        }

        #[test]
        fn serde_tokens() {
            for hardfork in VARIANTS {
                let json = serde_json::to_string(&hardfork).expect("serialization succeeds");
                let evm_json = serde_json::to_string(&EvmSpecId::from(hardfork))
                    .expect("serialization succeeds");
                assert_eq!(json, evm_json);
            }
        }

        #[test]
        fn defaults() {
            assert_eq!(EvmSpecId::from(L1Hardfork::default()), EvmSpecId::default());
        }
    }
}
