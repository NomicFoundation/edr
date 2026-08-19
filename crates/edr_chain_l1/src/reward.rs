//! Static block rewards, paid to the beneficiary of pre-merge L1 blocks.

use crate::L1Hardfork;

/// Returns the static block reward for the hardfork, or `None` post-merge.
pub fn miner_reward(hardfork: L1Hardfork) -> Option<u128> {
    match hardfork {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miner_rewards() {
        assert_eq!(
            miner_reward(L1Hardfork::BYZANTIUM),
            Some(3_000_000_000_000_000_000)
        );
        assert_eq!(
            miner_reward(L1Hardfork::CONSTANTINOPLE),
            Some(2_000_000_000_000_000_000)
        );
        assert_eq!(
            miner_reward(L1Hardfork::GRAY_GLACIER),
            Some(2_000_000_000_000_000_000)
        );
        assert_eq!(miner_reward(L1Hardfork::MERGE), None);
    }
}
