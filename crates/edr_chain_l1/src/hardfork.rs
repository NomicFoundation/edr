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
    Frontier = 0,
    /// Frontier Thawing hardfork
    #[strum(serialize = "Frontier Thawing")]
    FrontierThawing,
    /// Homestead hardfork
    Homestead,
    /// DAO Fork hardfork
    #[strum(serialize = "DAO Fork")]
    DaoFork,
    /// Tangerine Whistle hardfork
    Tangerine,
    /// Spurious Dragon hardfork
    #[strum(serialize = "Spurious")]
    SpuriousDragon,
    /// Byzantium hardfork
    Byzantium,
    /// Constantinople hardfork
    Constantinople,
    /// Petersburg hardfork
    Petersburg,
    /// Istanbul hardfork
    Istanbul,
    /// Muir Glacier hardfork
    MuirGlacier,
    /// Berlin hardfork
    Berlin,
    /// London hardfork
    London,
    /// Arrow Glacier hardfork
    #[strum(serialize = "Arrow Glacier")]
    ArrowGlacier,
    /// Gray Glacier hardfork
    #[strum(serialize = "Gray Glacier")]
    GrayGlacier,
    /// Paris/Merge hardfork
    Merge,
    /// Shanghai hardfork
    Shanghai,
    /// Cancun hardfork
    Cancun,
    /// Prague hardfork
    Prague,
    /// Osaka hardfork
    #[default]
    Osaka,
    /// Amsterdam hardfork
    Amsterdam,
}

fn unknown_hardfork(_name: &str) -> UnknownHardfork {
    UnknownHardfork
}

impl From<L1Hardfork> for EvmSpecId {
    fn from(hardfork: L1Hardfork) -> Self {
        match hardfork {
            // revm only models EVM behavior classes; hardforks without EVM
            // changes map to their EVM-equivalent predecessor.
            L1Hardfork::Frontier | L1Hardfork::FrontierThawing => EvmSpecId::FRONTIER,
            L1Hardfork::Homestead | L1Hardfork::DaoFork => EvmSpecId::HOMESTEAD,
            L1Hardfork::Tangerine => EvmSpecId::TANGERINE,
            L1Hardfork::SpuriousDragon => EvmSpecId::SPURIOUS_DRAGON,
            L1Hardfork::Byzantium => EvmSpecId::BYZANTIUM,
            // Constantinople never went live on mainnet on its own: Petersburg
            // (Constantinople minus EIP-1283) activated at the same block.
            L1Hardfork::Constantinople | L1Hardfork::Petersburg => EvmSpecId::PETERSBURG,
            L1Hardfork::Istanbul | L1Hardfork::MuirGlacier => EvmSpecId::ISTANBUL,
            L1Hardfork::Berlin => EvmSpecId::BERLIN,
            L1Hardfork::London | L1Hardfork::ArrowGlacier | L1Hardfork::GrayGlacier => {
                EvmSpecId::LONDON
            }
            L1Hardfork::Merge => EvmSpecId::MERGE,
            L1Hardfork::Shanghai => EvmSpecId::SHANGHAI,
            L1Hardfork::Cancun => EvmSpecId::CANCUN,
            L1Hardfork::Prague => EvmSpecId::PRAGUE,
            L1Hardfork::Osaka => EvmSpecId::OSAKA,
            L1Hardfork::Amsterdam => EvmSpecId::AMSTERDAM,
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
        L1Hardfork::Frontier,
        L1Hardfork::FrontierThawing,
        L1Hardfork::Homestead,
        L1Hardfork::DaoFork,
        L1Hardfork::Tangerine,
        L1Hardfork::SpuriousDragon,
        L1Hardfork::Byzantium,
        L1Hardfork::Constantinople,
        L1Hardfork::Petersburg,
        L1Hardfork::Istanbul,
        L1Hardfork::MuirGlacier,
        L1Hardfork::Berlin,
        L1Hardfork::London,
        L1Hardfork::ArrowGlacier,
        L1Hardfork::GrayGlacier,
        L1Hardfork::Merge,
        L1Hardfork::Shanghai,
        L1Hardfork::Cancun,
        L1Hardfork::Prague,
        L1Hardfork::Osaka,
        L1Hardfork::Amsterdam,
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
        assert_eq!(L1Hardfork::default(), L1Hardfork::Osaka);
    }

    #[test]
    fn evm_spec_id_conversion_is_monotonic() {
        for window in VARIANTS.windows(2) {
            assert!(EvmSpecId::from(window[0]) <= EvmSpecId::from(window[1]));
        }
    }

    #[test]
    fn defaults() {
        assert_eq!(EvmSpecId::from(L1Hardfork::default()), EvmSpecId::default());
    }
}
