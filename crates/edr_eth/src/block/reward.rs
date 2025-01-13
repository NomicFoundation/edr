use crate::{l1, U256};

/// Retrieves the miner reward for the provided hardfork.
pub fn miner_reward(hardfork: l1::Hardfork) -> Option<U256> {
    match hardfork {
        l1::Hardfork::Byzantium => Some(U256::from(3_000_000_000_000_000_000u128)),
        l1::Hardfork::Constantinople
        | l1::Hardfork::Petersburg
        | l1::Hardfork::Istanbul
        | l1::Hardfork::MuirGlacier
        | l1::Hardfork::Berlin
        | l1::Hardfork::London
        | l1::Hardfork::ArrowGlacier
        | l1::Hardfork::GrayGlacier => Some(U256::from(2_000_000_000_000_000_000u128)),
        _ => None,
    }
}
