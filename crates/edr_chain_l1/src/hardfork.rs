//! Ethereum L1 hardfork type, owned by EDR.

use core::str::FromStr;

use edr_chain_spec::{EvmSpecId, ProtocolParams};
use edr_primitives::UnknownHardfork;

/// Ethereum L1 hardfork.
///
/// Models protocol upgrades, including ones without EVM-semantics changes,
/// unlike [`EvmSpecId`] which models EVM behavior classes.
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
)]
pub enum L1Hardfork {
    /// Byzantium hardfork
    BYZANTIUM = 6,
    /// Constantinople hardfork
    CONSTANTINOPLE,
    /// Petersburg hardfork
    PETERSBURG,
    /// Istanbul hardfork
    ISTANBUL,
    /// Muir Glacier hardfork
    MUIR_GLACIER,
    /// Berlin hardfork
    BERLIN,
    /// London hardfork
    LONDON,
    /// Arrow Glacier hardfork
    ARROW_GLACIER,
    /// Gray Glacier hardfork
    GRAY_GLACIER,
    /// Paris/Merge hardfork
    MERGE,
    /// Shanghai hardfork
    SHANGHAI,
    /// Cancun hardfork
    CANCUN,
    /// Prague hardfork
    PRAGUE,
    /// Osaka hardfork
    #[default]
    OSAKA,
    /// Amsterdam hardfork
    AMSTERDAM,
}

impl From<L1Hardfork> for EvmSpecId {
    fn from(hardfork: L1Hardfork) -> Self {
        match hardfork {
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

impl FromStr for L1Hardfork {
    type Err = UnknownHardfork;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            name::BYZANTIUM => Ok(Self::BYZANTIUM),
            name::CONSTANTINOPLE => Ok(Self::CONSTANTINOPLE),
            name::PETERSBURG => Ok(Self::PETERSBURG),
            name::ISTANBUL => Ok(Self::ISTANBUL),
            name::MUIR_GLACIER => Ok(Self::MUIR_GLACIER),
            name::BERLIN => Ok(Self::BERLIN),
            name::LONDON => Ok(Self::LONDON),
            name::ARROW_GLACIER => Ok(Self::ARROW_GLACIER),
            name::GRAY_GLACIER => Ok(Self::GRAY_GLACIER),
            name::MERGE => Ok(Self::MERGE),
            name::SHANGHAI => Ok(Self::SHANGHAI),
            name::CANCUN => Ok(Self::CANCUN),
            name::PRAGUE => Ok(Self::PRAGUE),
            name::OSAKA => Ok(Self::OSAKA),
            name::AMSTERDAM => Ok(Self::AMSTERDAM),
            _ => Err(UnknownHardfork),
        }
    }
}

impl From<L1Hardfork> for &'static str {
    fn from(hardfork: L1Hardfork) -> Self {
        match hardfork {
            L1Hardfork::BYZANTIUM => name::BYZANTIUM,
            L1Hardfork::CONSTANTINOPLE => name::CONSTANTINOPLE,
            L1Hardfork::PETERSBURG => name::PETERSBURG,
            L1Hardfork::ISTANBUL => name::ISTANBUL,
            L1Hardfork::MUIR_GLACIER => name::MUIR_GLACIER,
            L1Hardfork::BERLIN => name::BERLIN,
            L1Hardfork::LONDON => name::LONDON,
            L1Hardfork::ARROW_GLACIER => name::ARROW_GLACIER,
            L1Hardfork::GRAY_GLACIER => name::GRAY_GLACIER,
            L1Hardfork::MERGE => name::MERGE,
            L1Hardfork::SHANGHAI => name::SHANGHAI,
            L1Hardfork::CANCUN => name::CANCUN,
            L1Hardfork::PRAGUE => name::PRAGUE,
            L1Hardfork::OSAKA => name::OSAKA,
            L1Hardfork::AMSTERDAM => name::AMSTERDAM,
        }
    }
}

impl ProtocolParams for L1Hardfork {
    fn bomb_delay(self) -> u64 {
        match self {
            L1Hardfork::BYZANTIUM => 3000000,
            L1Hardfork::CONSTANTINOPLE | L1Hardfork::PETERSBURG | L1Hardfork::ISTANBUL => 5000000,
            L1Hardfork::MUIR_GLACIER | L1Hardfork::BERLIN | L1Hardfork::LONDON => 9000000,
            // L1Hardfork::LONDON => 9500000, // EIP-3554
            L1Hardfork::ARROW_GLACIER => 10700000,
            L1Hardfork::GRAY_GLACIER => 11400000,
            _ => {
                unreachable!("Post-merge hardforks don't have a bomb delay")
            }
        }
    }

    fn miner_reward(self) -> Option<u128> {
        match self {
            L1Hardfork::BYZANTIUM => Some(3_000_000_000_000_000_000u128),
            L1Hardfork::CONSTANTINOPLE
            | L1Hardfork::PETERSBURG
            | L1Hardfork::ISTANBUL
            | L1Hardfork::MUIR_GLACIER
            | L1Hardfork::BERLIN
            | L1Hardfork::LONDON
            | L1Hardfork::ARROW_GLACIER
            | L1Hardfork::GRAY_GLACIER => Some(2_000_000_000_000_000_000u128),
            _ => None,
        }
    }
}

impl core::fmt::Display for L1Hardfork {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", <&'static str>::from(*self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VARIANTS: [L1Hardfork; 15] = [
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

    #[test]
    fn name_round_trip() {
        for hardfork in VARIANTS {
            let name: &'static str = hardfork.into();
            assert_eq!(hardfork.to_string(), name);
            assert_eq!(L1Hardfork::from_str(name), Ok(hardfork));
        }

        assert_eq!(L1Hardfork::from_str("Latest"), Err(UnknownHardfork));
        assert_eq!(L1Hardfork::from_str("NotAHardfork"), Err(UnknownHardfork));
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

    #[test]
    fn bomb_delays() {
        assert_eq!(L1Hardfork::BYZANTIUM.bomb_delay(), 3_000_000);
        assert_eq!(L1Hardfork::CONSTANTINOPLE.bomb_delay(), 5_000_000);
        assert_eq!(L1Hardfork::PETERSBURG.bomb_delay(), 5_000_000);
        assert_eq!(L1Hardfork::ISTANBUL.bomb_delay(), 5_000_000);
        assert_eq!(L1Hardfork::MUIR_GLACIER.bomb_delay(), 9_000_000);
        assert_eq!(L1Hardfork::BERLIN.bomb_delay(), 9_000_000);
        assert_eq!(L1Hardfork::LONDON.bomb_delay(), 9_000_000);
        assert_eq!(L1Hardfork::ARROW_GLACIER.bomb_delay(), 10_700_000);
        assert_eq!(L1Hardfork::GRAY_GLACIER.bomb_delay(), 11_400_000);
    }

    #[test]
    fn miner_rewards() {
        assert_eq!(
            L1Hardfork::BYZANTIUM.miner_reward(),
            Some(3_000_000_000_000_000_000)
        );
        assert_eq!(
            L1Hardfork::CONSTANTINOPLE.miner_reward(),
            Some(2_000_000_000_000_000_000)
        );
        assert_eq!(
            L1Hardfork::GRAY_GLACIER.miner_reward(),
            Some(2_000_000_000_000_000_000)
        );
        assert_eq!(L1Hardfork::MERGE.miner_reward(), None);
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
