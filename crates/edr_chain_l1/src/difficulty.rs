//! Ethash canonical difficulty, used by the L1 hardforks that precede the
//! merge.

use edr_block_header::BlockHeader;
use edr_primitives::{KECCAK_RLP_EMPTY_ARRAY, U256};

use crate::L1Hardfork;

/// The L1 hardforks that precede the merge, and therefore mine blocks using
/// Ethash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreMergeL1Hardfork {
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
    ArrowGlacier,
    /// Gray Glacier hardfork
    GrayGlacier,
}

impl TryFrom<L1Hardfork> for PreMergeL1Hardfork {
    /// The post-merge hardfork that could not be converted.
    type Error = L1Hardfork;

    fn try_from(hardfork: L1Hardfork) -> Result<Self, Self::Error> {
        match hardfork {
            L1Hardfork::BYZANTIUM => Ok(Self::Byzantium),
            L1Hardfork::CONSTANTINOPLE => Ok(Self::Constantinople),
            L1Hardfork::PETERSBURG => Ok(Self::Petersburg),
            L1Hardfork::ISTANBUL => Ok(Self::Istanbul),
            L1Hardfork::MUIR_GLACIER => Ok(Self::MuirGlacier),
            L1Hardfork::BERLIN => Ok(Self::Berlin),
            L1Hardfork::LONDON => Ok(Self::London),
            L1Hardfork::ARROW_GLACIER => Ok(Self::ArrowGlacier),
            L1Hardfork::GRAY_GLACIER => Ok(Self::GrayGlacier),
            hardfork => Err(hardfork),
        }
    }
}

/// Returns the difficulty bomb delay for the hardfork, as introduced by EIPs
/// 649, 1234, 2384, 4345 and 5133.
pub fn bomb_delay(hardfork: PreMergeL1Hardfork) -> u64 {
    use PreMergeL1Hardfork::{
        ArrowGlacier, Berlin, Byzantium, Constantinople, GrayGlacier, Istanbul, London,
        MuirGlacier, Petersburg,
    };

    match hardfork {
        Byzantium => 3_000_000,
        Constantinople | Petersburg | Istanbul => 5_000_000,
        MuirGlacier | Berlin | London => 9_000_000,
        // LONDON => 9_500_000, // EIP-3554
        ArrowGlacier => 10_700_000,
        GrayGlacier => 11_400_000,
    }
}

/// Calculates the mining difficulty of a block.
pub fn calculate_ethash_canonical_difficulty(
    hardfork: PreMergeL1Hardfork,
    parent: &BlockHeader,
    block_number: u64,
    block_timestamp: u64,
    min_ethash_difficulty: u64,
) -> U256 {
    let bound_divisor = U256::from(2048);
    let offset = parent.difficulty / bound_divisor;

    let mut difficulty = {
        let uncle_addend = if parent.ommers_hash == KECCAK_RLP_EMPTY_ARRAY {
            1
        } else {
            2
        };
        let a = (block_timestamp - parent.timestamp) / 9;

        if let Some(a) = a.checked_sub(uncle_addend) {
            let a = U256::from(a.min(99));

            parent.difficulty - a * offset
        } else {
            let a = U256::from(uncle_addend - a);
            parent.difficulty + a * offset
        }
    };

    if let Some(exp) = block_number
        .checked_sub(bomb_delay(hardfork))
        .and_then(|num| (num / 100000).checked_sub(2))
    {
        difficulty += U256::from(2u64).pow(U256::from(exp));
    }

    difficulty.max(U256::from(min_ethash_difficulty))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bomb_delays() {
        assert_eq!(bomb_delay(PreMergeL1Hardfork::Byzantium), 3_000_000);
        assert_eq!(bomb_delay(PreMergeL1Hardfork::Constantinople), 5_000_000);
        assert_eq!(bomb_delay(PreMergeL1Hardfork::Petersburg), 5_000_000);
        assert_eq!(bomb_delay(PreMergeL1Hardfork::Istanbul), 5_000_000);
        assert_eq!(bomb_delay(PreMergeL1Hardfork::MuirGlacier), 9_000_000);
        assert_eq!(bomb_delay(PreMergeL1Hardfork::Berlin), 9_000_000);
        assert_eq!(bomb_delay(PreMergeL1Hardfork::London), 9_000_000);
        assert_eq!(bomb_delay(PreMergeL1Hardfork::ArrowGlacier), 10_700_000);
        assert_eq!(bomb_delay(PreMergeL1Hardfork::GrayGlacier), 11_400_000);
    }

    #[test]
    fn try_from_accepts_every_pre_merge_hardfork() {
        const PRE_MERGE: [(L1Hardfork, PreMergeL1Hardfork); 9] = [
            (L1Hardfork::BYZANTIUM, PreMergeL1Hardfork::Byzantium),
            (
                L1Hardfork::CONSTANTINOPLE,
                PreMergeL1Hardfork::Constantinople,
            ),
            (L1Hardfork::PETERSBURG, PreMergeL1Hardfork::Petersburg),
            (L1Hardfork::ISTANBUL, PreMergeL1Hardfork::Istanbul),
            (L1Hardfork::MUIR_GLACIER, PreMergeL1Hardfork::MuirGlacier),
            (L1Hardfork::BERLIN, PreMergeL1Hardfork::Berlin),
            (L1Hardfork::LONDON, PreMergeL1Hardfork::London),
            (L1Hardfork::ARROW_GLACIER, PreMergeL1Hardfork::ArrowGlacier),
            (L1Hardfork::GRAY_GLACIER, PreMergeL1Hardfork::GrayGlacier),
        ];

        for (hardfork, expected) in PRE_MERGE {
            assert_eq!(PreMergeL1Hardfork::try_from(hardfork), Ok(expected));
        }
    }

    #[test]
    fn try_from_rejects_post_merge_hardforks() {
        const POST_MERGE: [L1Hardfork; 6] = [
            L1Hardfork::MERGE,
            L1Hardfork::SHANGHAI,
            L1Hardfork::CANCUN,
            L1Hardfork::PRAGUE,
            L1Hardfork::OSAKA,
            L1Hardfork::AMSTERDAM,
        ];

        for hardfork in POST_MERGE {
            assert_eq!(PreMergeL1Hardfork::try_from(hardfork), Err(hardfork));
        }
    }
}
