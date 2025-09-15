// Part of this code was adapted from foundry and is distributed under their
// licenss:
// - https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/LICENSE-APACHE
// - https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/LICENSE-MIT
// For the original context see: https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/anvil/core/src/eth/utils.rs
//
// Part of this code was adapted from ethers-rs and is distributed under their
// licenss:
// - https://github.com/gakonst/ethers-rs/blob/cba6f071aedafb766e82e4c2f469ed5e4638337d/LICENSE-APACHE
// - https://github.com/gakonst/ethers-rs/blob/cba6f071aedafb766e82e4c2f469ed5e4638337d/LICENSE-MIT
// For the original context see: https://github.com/gakonst/ethers-rs/blob/cba6f071aedafb766e82e4c2f469ed5e4638337d/ethers-core/src/utils/hash.rs

use edr_primitives::U256;

/// Convert a U256 to String as a 32-byte 0x prefixed hex string.
pub fn u256_to_padded_hex(word: &U256) -> String {
    if word == &U256::ZERO {
        // For 0 zero, the #066x formatter doesn't add padding.
        format!("0x{}", "0".repeat(64))
    } else {
        // 66 = 64 hex chars + 0x prefix
        format!("{word:#066x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u256_to_padded_hex() {
        assert_eq!(
            u256_to_padded_hex(&U256::ZERO),
            "0x0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(
            u256_to_padded_hex(&U256::from(1)),
            "0x0000000000000000000000000000000000000000000000000000000000000001"
        );
    }
}
