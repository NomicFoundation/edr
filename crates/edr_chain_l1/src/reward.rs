//! Static block rewards, paid to the beneficiary of pre-merge L1 blocks.

use crate::L1Hardfork;

/// Returns the static block reward for the hardfork, or `None` post-merge.
pub fn miner_reward(hardfork: L1Hardfork) -> Option<u128> {
    match hardfork {
        L1Hardfork::Byzantium => Some(3_000_000_000_000_000_000u128),
        L1Hardfork::Constantinople
        | L1Hardfork::Petersburg
        | L1Hardfork::Istanbul
        | L1Hardfork::MuirGlacier
        | L1Hardfork::Berlin
        | L1Hardfork::London
        | L1Hardfork::ArrowGlacier
        | L1Hardfork::GrayGlacier => Some(2_000_000_000_000_000_000u128),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miner_rewards() {
        assert_eq!(
            miner_reward(L1Hardfork::Byzantium),
            Some(3_000_000_000_000_000_000)
        );
        assert_eq!(
            miner_reward(L1Hardfork::Constantinople),
            Some(2_000_000_000_000_000_000)
        );
        assert_eq!(
            miner_reward(L1Hardfork::GrayGlacier),
            Some(2_000_000_000_000_000_000)
        );
        assert_eq!(miner_reward(L1Hardfork::Merge), None);
    }
}
