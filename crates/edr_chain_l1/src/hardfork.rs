//! Ethereum L1 hardfork type, owned by EDR.

use edr_chain_spec::EvmSpecId;
use edr_primitives::UnknownHardfork;

/// Ethereum L1 hardfork.
///
/// Models protocol upgrades, including ones without EVM-semantics changes,
/// unlike [`EvmSpecId`] which models EVM behavior classes.
///
/// The strum-derived names (`serialize_all = "camelCase"`) are public API;
/// the expected strings are pinned in this module's tests.
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
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
)]
#[strum(serialize_all = "camelCase", parse_err_ty = UnknownHardfork, parse_err_fn = unknown_hardfork)]
pub enum L1Hardfork {
    /// Byzantium hardfork
    Byzantium = 6,
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
    ArrowGlacier,
    /// Gray Glacier hardfork
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

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use super::*;

    const VARIANTS: [L1Hardfork; 15] = [
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

    /// The public hardfork name strings. Changing one is a breaking change
    /// for consumers.
    const NAMES: [&str; 15] = [
        "byzantium",
        "constantinople",
        "petersburg",
        "istanbul",
        "muirGlacier",
        "berlin",
        "london",
        "arrowGlacier",
        "grayGlacier",
        "merge",
        "shanghai",
        "cancun",
        "prague",
        "osaka",
        "amsterdam",
    ];

    #[test]
    fn name_round_trip() {
        for (hardfork, name) in VARIANTS.into_iter().zip(NAMES) {
            assert_eq!(hardfork.to_string(), name);
            assert_eq!(<&'static str>::from(hardfork), name);
            assert_eq!(L1Hardfork::from_str(name), Ok(hardfork));
        }

        assert_eq!(L1Hardfork::from_str("latest"), Err(UnknownHardfork));
        assert_eq!(L1Hardfork::from_str("NotAHardfork"), Err(UnknownHardfork));
        // strum must not fall back to parsing variant identifiers.
        assert_eq!(L1Hardfork::from_str("MUIR_GLACIER"), Err(UnknownHardfork));
        // Former (PascalCase) names must no longer parse.
        assert_eq!(L1Hardfork::from_str("MuirGlacier"), Err(UnknownHardfork));
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
