// Part of this code was adapted from foundry and is distributed under their
// licenss:
// - https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/LICENSE-APACHE
// - https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/LICENSE-MIT
// For the original context see: https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/anvil/core/src/eth/block.rs

mod difficulty;
mod overrides;
mod reorg;
mod reward;

use alloy_rlp::{BufMut, Decodable, RlpDecodable, RlpEncodable};
pub use revm_context_interface::Block;

use self::difficulty::calculate_ethash_canonical_difficulty;
pub use self::{
    overrides::HeaderOverrides,
    reorg::{
        block_time, is_safe_block_number, largest_safe_block_number, safe_block_depth,
        IsSafeBlockNumberArgs, LargestSafeBlockNumberArgs,
    },
    reward::miner_reward,
};
use crate::{
    b256,
    eips::{eip1559::ConstantBaseFeeParams, eip4844, eip7691},
    keccak256, l1,
    spec::EthHeaderConstants,
    trie::{self, KECCAK_NULL_RLP},
    withdrawal::Withdrawal,
    Address, Bloom, Bytes, B256, B64, U256,
};

/// ethereum block header
#[derive(Clone, Debug, Default, PartialEq, Eq, RlpDecodable, RlpEncodable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[rlp(trailing)]
pub struct Header {
    /// The parent block's hash
    pub parent_hash: B256,
    /// The ommers' root hash
    pub ommers_hash: B256,
    /// The block's beneficiary address
    pub beneficiary: Address,
    /// The state's root hash
    pub state_root: B256,
    /// The transactions' root hash
    pub transactions_root: B256,
    /// The receipts' root hash
    pub receipts_root: B256,
    /// The logs' bloom
    pub logs_bloom: Bloom,
    /// The block's difficulty
    pub difficulty: U256,
    /// The block's number
    pub number: u64,
    /// The block's gas limit
    pub gas_limit: u64,
    /// The amount of gas used by the block
    pub gas_used: u64,
    /// The block's timestamp
    pub timestamp: u64,
    /// The block's extra data
    pub extra_data: Bytes,
    /// The block's mix hash
    pub mix_hash: B256,
    /// The block's nonce
    pub nonce: B64,
    /// `BaseFee` was added by EIP-1559 and is ignored in legacy headers.
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity::opt"))]
    pub base_fee_per_gas: Option<u128>,
    /// `WithdrawalsHash` was added by EIP-4895 and is ignored in legacy
    /// headers.
    pub withdrawals_root: Option<B256>,
    /// Blob gas was added by EIP-4844 and is ignored in older headers.
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub blob_gas: Option<BlobGas>,
    /// The hash tree root of the parent beacon block for the given execution
    /// block (EIP-4788).
    pub parent_beacon_block_root: Option<B256>,
    /// The commitment hash calculated for a list of [EIP-7685] data requests.
    ///
    /// [EIP-7685]: https://eips.ethereum.org/EIPS/eip-7685
    pub requests_hash: Option<B256>,
}

/// Information about the blob gas used in a block.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct BlobGas {
    /// The total amount of blob gas consumed by the transactions within the
    /// block.
    pub gas_used: u64,
    /// The running total of blob gas consumed in excess of the target, prior to
    /// the block. Blocks with above-target blob gas consumption increase this
    /// value, blocks with below-target blob gas consumption decrease it
    /// (bounded at 0).
    pub excess_gas: u64,
}

// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl Decodable for BlobGas {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let blob_gas = Self {
            gas_used: u64::decode(buf)?,
            excess_gas: u64::decode(buf)?,
        };

        Ok(blob_gas)
    }
}

// We need a custom implementation to avoid the struct being treated as an RLP
// list.
impl alloy_rlp::Encodable for BlobGas {
    fn encode(&self, out: &mut dyn BufMut) {
        self.gas_used.encode(out);
        self.excess_gas.encode(out);
    }

    fn length(&self) -> usize {
        self.gas_used.length() + self.excess_gas.length()
    }
}

impl Header {
    /// Constructs a header from the provided [`PartialHeader`] and hashtree
    /// root of the transactions.
    pub fn new(partial_header: PartialHeader, transactions_root: B256) -> Self {
        Self {
            parent_hash: partial_header.parent_hash,
            ommers_hash: partial_header.ommers_hash,
            beneficiary: partial_header.beneficiary,
            state_root: partial_header.state_root,
            transactions_root,
            receipts_root: partial_header.receipts_root,
            logs_bloom: partial_header.logs_bloom,
            difficulty: partial_header.difficulty,
            number: partial_header.number,
            gas_limit: partial_header.gas_limit,
            gas_used: partial_header.gas_used,
            timestamp: partial_header.timestamp,
            extra_data: partial_header.extra_data,
            mix_hash: partial_header.mix_hash,
            nonce: partial_header.nonce,
            base_fee_per_gas: partial_header.base_fee,
            withdrawals_root: partial_header.withdrawals_root,
            blob_gas: partial_header.blob_gas,
            parent_beacon_block_root: partial_header.parent_beacon_block_root,
            requests_hash: partial_header.requests_hash,
        }
    }

    /// Calculates the block's hash.
    pub fn hash(&self) -> B256 {
        let encoded = alloy_rlp::encode(self);
        keccak256(encoded)
    }
}

/// Partial header definition without ommers hash and transactions root
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialHeader {
    /// The parent block's hash
    pub parent_hash: B256,
    /// The ommers' root hash
    pub ommers_hash: B256,
    /// The block's beneficiary address
    pub beneficiary: Address,
    /// The state's root hash
    pub state_root: B256,
    /// The receipts' root hash
    pub receipts_root: B256,
    /// The logs' bloom
    pub logs_bloom: Bloom,
    /// The block's difficulty
    pub difficulty: U256,
    /// The block's number
    pub number: u64,
    /// The block's gas limit
    pub gas_limit: u64,
    /// The amount of gas used by the block
    pub gas_used: u64,
    /// The block's timestamp
    pub timestamp: u64,
    /// The block's extra data
    pub extra_data: Bytes,
    /// The block's mix hash
    pub mix_hash: B256,
    /// The block's nonce
    pub nonce: B64,
    /// `BaseFee` was added by EIP-1559 and is ignored in legacy headers.
    pub base_fee: Option<u128>,
    /// `WithdrawalsHash` was added by EIP-4895 and is ignored in legacy
    /// headers.
    pub withdrawals_root: Option<B256>,
    /// Blob gas was added by EIP-4844 and is ignored in older headers.
    pub blob_gas: Option<BlobGas>,
    /// The hash tree root of the parent beacon block for the given execution
    /// block (EIP-4788).
    pub parent_beacon_block_root: Option<B256>,
    /// The commitment hash calculated for a list of [EIP-7685] data requests.
    ///
    /// [EIP-7685]: https://eips.ethereum.org/EIPS/eip-7685
    pub requests_hash: Option<B256>,
}

impl PartialHeader {
    /// Constructs a new instance based on the provided [`HeaderOverrides`] and
    /// parent [`Header`] for the given [`l1::SpecId`].
    pub fn new<ChainSpecT: EthHeaderConstants>(
        hardfork: ChainSpecT::Hardfork,
        overrides: HeaderOverrides,
        parent: Option<&Header>,
        ommers: &Vec<Header>,
        withdrawals: Option<&Vec<Withdrawal>>,
    ) -> Self {
        let timestamp = overrides.timestamp.unwrap_or_default();
        let number = overrides.number.unwrap_or({
            if let Some(parent) = &parent {
                parent.number + 1
            } else {
                0
            }
        });

        let parent_hash = overrides.parent_hash.unwrap_or_else(|| {
            if let Some(parent) = parent {
                parent.hash()
            } else {
                B256::ZERO
            }
        });

        Self {
            parent_hash,
            ommers_hash: keccak256(alloy_rlp::encode(ommers)),
            beneficiary: overrides.beneficiary.unwrap_or_default(),
            state_root: overrides.state_root.unwrap_or(KECCAK_NULL_RLP),
            receipts_root: KECCAK_NULL_RLP,
            logs_bloom: Bloom::default(),
            difficulty: overrides.difficulty.unwrap_or_else(|| {
                if hardfork.into() >= l1::SpecId::MERGE {
                    U256::ZERO
                } else if let Some(parent) = parent {
                    calculate_ethash_canonical_difficulty::<ChainSpecT>(
                        hardfork.into(),
                        parent,
                        number,
                        timestamp,
                    )
                } else {
                    U256::from(1)
                }
            }),
            number,
            gas_limit: overrides.gas_limit.unwrap_or(1_000_000),
            gas_used: 0,
            timestamp,
            extra_data: overrides.extra_data.unwrap_or_default(),
            mix_hash: overrides.mix_hash.unwrap_or_default(),
            nonce: overrides.nonce.unwrap_or_else(|| {
                if hardfork.into() >= l1::SpecId::MERGE {
                    B64::ZERO
                } else {
                    B64::from(66u64)
                }
            }),
            base_fee: overrides.base_fee.or_else(|| {
                if hardfork.into() >= l1::SpecId::LONDON {
                    Some(if let Some(parent) = &parent {
                        if let Some(base_fee_params) = &overrides.base_fee_params {
                            calculate_next_base_fee_per_gas(parent, base_fee_params)
                        } else {
                            calculate_next_base_fee_per_gas_for_chain_spec::<ChainSpecT>(
                                hardfork, parent,
                            )
                        }
                    } else {
                        u128::from(alloy_eips::eip1559::INITIAL_BASE_FEE)
                    })
                } else {
                    None
                }
            }),
            withdrawals_root: overrides.withdrawals_root.or_else(|| {
                if hardfork.into() >= l1::SpecId::SHANGHAI {
                    let withdrawals_root = withdrawals.map_or(KECCAK_NULL_RLP, |withdrawals| {
                        trie::ordered_trie_root(withdrawals.iter().map(alloy_rlp::encode))
                    });

                    Some(withdrawals_root)
                } else {
                    None
                }
            }),
            blob_gas: overrides.blob_gas.or_else(|| {
                if hardfork.into() >= l1::SpecId::CANCUN {
                    let excess_gas = parent.and_then(|parent| parent.blob_gas.as_ref()).map_or(
                        // For the first (post-fork) block, both parent.blob_gas_used and
                        // parent.excess_blob_gas are evaluated as 0.
                        0,
                        |BlobGas {
                             gas_used,
                             excess_gas,
                         }| {
                            let target_blob_number_per_blob =
                                if hardfork.into() >= l1::SpecId::PRAGUE {
                                    eip7691::TARGET_BLOBS_PER_BLOCK_ELECTRA
                                } else {
                                    eip4844::TARGET_BLOBS_PER_BLOCK
                                };

                            let target_blob_gas_per_block =
                                eip4844::GAS_PER_BLOB * target_blob_number_per_blob;

                            eip4844::calc_excess_blob_gas(
                                *excess_gas,
                                *gas_used,
                                target_blob_gas_per_block,
                            )
                        },
                    );

                    Some(BlobGas {
                        gas_used: 0,
                        excess_gas,
                    })
                } else {
                    None
                }
            }),
            parent_beacon_block_root: overrides.parent_beacon_block_root.or_else(|| {
                if hardfork.into() >= l1::SpecId::CANCUN {
                    // Initial value from https://eips.ethereum.org/EIPS/eip-4788
                    Some(B256::ZERO)
                } else {
                    None
                }
            }),
            requests_hash: overrides.requests_hash.or_else(|| {
                if hardfork.into() >= l1::SpecId::PRAGUE {
                    // sha("") for an empty list of requests
                    Some(b256!(
                        "0xe3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    ))
                } else {
                    None
                }
            }),
        }
    }
}

impl From<Header> for PartialHeader {
    fn from(header: Header) -> PartialHeader {
        Self {
            parent_hash: header.parent_hash,
            ommers_hash: header.ommers_hash,
            beneficiary: header.beneficiary,
            state_root: header.state_root,
            receipts_root: header.receipts_root,
            logs_bloom: header.logs_bloom,
            difficulty: header.difficulty,
            number: header.number,
            gas_limit: header.gas_limit,
            gas_used: header.gas_used,
            timestamp: header.timestamp,
            extra_data: header.extra_data,
            mix_hash: header.mix_hash,
            nonce: header.nonce,
            base_fee: header.base_fee_per_gas,
            withdrawals_root: header.withdrawals_root,
            blob_gas: header.blob_gas,
            parent_beacon_block_root: header.parent_beacon_block_root,
            requests_hash: header.requests_hash,
        }
    }
}

/// Calculates the next base fee per gas for a post-London block, given the
/// parent's header and the base fee parameters provided by
/// [`EthHeaderConstants`].
///
/// # Panics
///
/// Panics if the parent header does not contain a base fee.
pub fn calculate_next_base_fee_per_gas_for_chain_spec<ChainSpecT: EthHeaderConstants>(
    hardfork: ChainSpecT::Hardfork,
    parent: &Header,
) -> u128 {
    let base_fee_params = ChainSpecT::BASE_FEE_PARAMS
        .at_hardfork(hardfork)
        .expect("Chain spec must have base fee params for post-London hardforks");

    calculate_next_base_fee_per_gas(parent, base_fee_params)
}

/// Calculates the next base fee for a post-London block, given the parent's
/// header.
///
/// # Panics
///
/// Panics if the parent header does not contain a base fee.
pub fn calculate_next_base_fee_per_gas(
    parent: &Header,
    base_fee_params: &ConstantBaseFeeParams,
) -> u128 {
    // Adapted from https://github.com/alloy-rs/alloy/blob/main/crates/eips/src/eip1559/helpers.rs#L41
    // modifying it to support `u128`.
    // TODO: Remove once https://github.com/alloy-rs/alloy/issues/2181 has been addressed.
    let gas_used = u128::from(parent.gas_used);
    let gas_limit = u128::from(parent.gas_limit);

    // In reality, [EIP-1559] specifies an initial base fee block number at which to
    // use the initial base fee, but we always use it if the parent block is
    // missing the base fee.
    //
    // [EIP-1559]: https://eips.ethereum.org/EIPS/eip-1559
    let base_fee = parent
        .base_fee_per_gas
        .unwrap_or(u128::from(alloy_eips::eip1559::INITIAL_BASE_FEE));

    // Calculate the target gas by dividing the gas limit by the elasticity
    // multiplier.
    let gas_target = gas_limit / base_fee_params.elasticity_multiplier;

    match gas_used.cmp(&gas_target) {
        // If the gas used in the current block is equal to the gas target, the base fee remains the
        // same (no increase).
        core::cmp::Ordering::Equal => base_fee,
        // If the gas used in the current block is greater than the gas target, calculate a new
        // increased base fee.
        core::cmp::Ordering::Greater => {
            // Calculate the increase in base fee based on the formula defined by EIP-1559.
            base_fee
                + core::cmp::max(
                    // Ensure a minimum increase of 1.
                    1,
                    base_fee * (gas_used - gas_target)
                        / (gas_target * base_fee_params.max_change_denominator),
                )
        }
        // If the gas used in the current block is less than the gas target, calculate a new
        // decreased base fee.
        core::cmp::Ordering::Less => {
            // Calculate the decrease in base fee based on the formula defined by EIP-1559.
            base_fee.saturating_sub(
                base_fee * (gas_target - gas_used)
                    / (gas_target * base_fee_params.max_change_denominator),
            )
        }
    }
}

/// Calculates the next base fee per blob gas for a post-Cancun block, given the
/// parent's header.
pub fn calculate_next_base_fee_per_blob_gas<HardforkT: Into<l1::SpecId>>(
    parent: &Header,
    hardfork: HardforkT,
) -> u128 {
    parent
        .blob_gas
        .as_ref()
        .map_or(0u128, |BlobGas { excess_gas, .. }| {
            eip4844::calc_blob_gasprice(*excess_gas, hardfork.into() >= l1::SpecId::PRAGUE)
        })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::trie::KECCAK_RLP_EMPTY_ARRAY;

    #[test]
    fn header_rlp_roundtrip() {
        let mut header = Header {
            parent_hash: B256::default(),
            ommers_hash: B256::default(),
            beneficiary: Address::default(),
            state_root: B256::default(),
            transactions_root: B256::default(),
            receipts_root: B256::default(),
            logs_bloom: Bloom::default(),
            difficulty: U256::default(),
            number: 124,
            gas_limit: u64::default(),
            gas_used: 1337,
            timestamp: 0,
            extra_data: Bytes::default(),
            mix_hash: B256::default(),
            nonce: B64::from(99u64),
            base_fee_per_gas: None,
            withdrawals_root: None,
            blob_gas: None,
            parent_beacon_block_root: None,
            requests_hash: Some(B256::random()),
        };

        let encoded = alloy_rlp::encode(&header);
        let decoded = Header::decode(&mut encoded.as_slice()).unwrap();
        assert_eq!(header, decoded);

        header.base_fee_per_gas = Some(12345);

        let encoded = alloy_rlp::encode(&header);
        let decoded = Header::decode(&mut encoded.as_slice()).unwrap();
        assert_eq!(header, decoded);
    }

    #[test]
    // Test vector from: https://eips.ethereum.org/EIPS/eip-2481
    fn test_encode_block_header() {
        let expected = hex::decode("f901f9a00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000940000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008208ae820d0582115c8215b3821a0a827788a00000000000000000000000000000000000000000000000000000000000000000880000000000000000").unwrap();

        let header = Header {
            parent_hash: B256::ZERO,
            ommers_hash: B256::ZERO,
            beneficiary: Address::ZERO,
            state_root: B256::ZERO,
            transactions_root: B256::ZERO,
            receipts_root: B256::ZERO,
            logs_bloom: Bloom::ZERO,
            difficulty: U256::from(0x8aeu64),
            number: 0xd05u64,
            gas_limit: 0x115cu64,
            gas_used: 0x15b3u64,
            timestamp: 0x1a0au64,
            extra_data: hex::decode("7788").unwrap().into(),
            mix_hash: B256::ZERO,
            nonce: B64::ZERO,
            base_fee_per_gas: None,
            withdrawals_root: None,
            blob_gas: None,
            parent_beacon_block_root: None,
            requests_hash: None,
        };
        let encoded = alloy_rlp::encode(&header);
        assert_eq!(encoded, expected);
    }

    #[test]
    // Test vector from: https://github.com/ethereum/tests/blob/f47bbef4da376a49c8fc3166f09ab8a6d182f765/BlockchainTests/ValidBlocks/bcEIP1559/baseFee.json#L15-L36
    fn test_eip1559_block_header_hash() {
        let expected_hash =
            B256::from_str("0x6a251c7c3c5dca7b42407a3752ff48f3bbca1fab7f9868371d9918daf1988d1f")
                .unwrap();
        let header = Header {
            parent_hash: B256::from_str(
                "0xe0a94a7a3c9617401586b1a27025d2d9671332d22d540e0af72b069170380f2a",
            )
            .unwrap(),
            ommers_hash: B256::from_str(
                "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            )
            .unwrap(),
            beneficiary: Address::from_str("0xba5e000000000000000000000000000000000000").unwrap(),
            state_root: B256::from_str(
                "0xec3c94b18b8a1cff7d60f8d258ec723312932928626b4c9355eb4ab3568ec7f7",
            )
            .unwrap(),
            transactions_root: B256::from_str(
                "0x50f738580ed699f0469702c7ccc63ed2e51bc034be9479b7bff4e68dee84accf",
            )
            .unwrap(),
            receipts_root: B256::from_str(
                "0x29b0562f7140574dd0d50dee8a271b22e1a0a7b78fca58f7c60370d8317ba2a9",
            )
            .unwrap(),
            logs_bloom: Bloom::ZERO,
            difficulty: U256::from(0x020000u64),
            number: 0x01,
            gas_limit: 0x016345785d8a0000,
            gas_used: 0x015534,
            timestamp: 0x079e,
            extra_data: hex::decode("42").unwrap().into(),
            mix_hash: B256::ZERO,
            nonce: B64::ZERO,
            base_fee_per_gas: Some(0x036b),
            withdrawals_root: None,
            blob_gas: None,
            parent_beacon_block_root: None,
            requests_hash: None,
        };
        assert_eq!(header.hash(), expected_hash);
    }

    #[test]
    // Test vector from: https://eips.ethereum.org/EIPS/eip-2481
    fn test_decode_block_header() {
        let data = hex::decode("f901f9a00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000940000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008208ae820d0582115c8215b3821a0a827788a00000000000000000000000000000000000000000000000000000000000000000880000000000000000").unwrap();

        let expected = Header {
            parent_hash: B256::ZERO,
            ommers_hash: B256::ZERO,
            beneficiary: Address::ZERO,
            state_root: B256::ZERO,
            transactions_root: B256::ZERO,
            receipts_root: B256::ZERO,
            logs_bloom: Bloom::ZERO,
            difficulty: U256::from(0x8aeu64),
            number: 0xd05u64,
            gas_limit: 0x115cu64,
            gas_used: 0x15b3u64,
            timestamp: 0x1a0au64,
            extra_data: hex::decode("7788").unwrap().into(),
            mix_hash: B256::ZERO,
            nonce: B64::ZERO,
            base_fee_per_gas: None,
            withdrawals_root: None,
            blob_gas: None,
            parent_beacon_block_root: None,
            requests_hash: None,
        };
        let decoded = Header::decode(&mut data.as_slice()).unwrap();
        assert_eq!(decoded, expected);
    }

    // Test vector from https://github.com/ethereum/tests/blob/a33949df17a1c382ffee5666e66d26bde7a089f9/EIPTests/Pyspecs/cancun/eip4844_blobs/correct_increasing_blob_gas_costs.json#L16
    #[test]
    fn block_header_rlp_encoding_cancun() {
        let expected_encoding = hex::decode("f90242a0258811d02512e87e09253a948330eff05da06b7656143a211fa3687901217f57a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347942adc25665018aa1fe0e6bc666dac8fc2697ff9baa06a086c92bb1d4ee6dc4ca73e66529037591bd4d6590350f6c904bc78dc21b75ca0dc387fc6ef9e3eb53baa85df89a1f9b91a4a9ab472ee7e928b4b7fdc06dfa5d1a0eaa8c40899a61ae59615cf9985f5e2194f8fd2b57d273be63bde6733e89b12abb9010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000800188016345785d8a00008252080c80a0000000000000000000000000000000000000000000000000000000000000000088000000000000000007a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b4218308000083220000a00000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let expected_hash =
            B256::from_str("0xd2caf87ef0ecbbf1d8721e4f63d56b3a5b4bf8b5faa0409aa6b99a729affe346")
                .unwrap();

        let header = Header {
            base_fee_per_gas: Some(0x07),
            blob_gas: Some(BlobGas {
                gas_used: 0x080000u64,
                excess_gas: 0x220000u64,
            }),
            logs_bloom: Bloom::ZERO,
            beneficiary: Address::from_str("0x2adc25665018aa1fe0e6bc666dac8fc2697ff9ba").unwrap(),
            difficulty: U256::ZERO,
            extra_data: Bytes::default(),
            gas_limit: 0x016345785d8a0000u64,
            gas_used: 0x5208u64,
            mix_hash: B256::ZERO,
            nonce: B64::ZERO,
            number: 0x01u64,
            parent_beacon_block_root: Some(B256::ZERO),
            parent_hash: B256::from_str(
                "0x258811d02512e87e09253a948330eff05da06b7656143a211fa3687901217f57",
            )
            .unwrap(),
            receipts_root: B256::from_str(
                "0xeaa8c40899a61ae59615cf9985f5e2194f8fd2b57d273be63bde6733e89b12ab",
            )
            .unwrap(),
            state_root: B256::from_str(
                "0x6a086c92bb1d4ee6dc4ca73e66529037591bd4d6590350f6c904bc78dc21b75c",
            )
            .unwrap(),
            timestamp: 0x0cu64,
            transactions_root: B256::from_str(
                "0xdc387fc6ef9e3eb53baa85df89a1f9b91a4a9ab472ee7e928b4b7fdc06dfa5d1",
            )
            .unwrap(),
            ommers_hash: KECCAK_RLP_EMPTY_ARRAY,
            withdrawals_root: Some(KECCAK_NULL_RLP),
            requests_hash: None,
        };

        let encoded = alloy_rlp::encode(&header);
        assert_eq!(encoded, expected_encoding);
        assert_eq!(header.hash(), expected_hash);
    }

    // Test vector from https://github.com/ethereum/tests/blob/c67e485ff8b5be9abc8ad15345ec21aa22e290d9/BlockchainTests/ValidBlocks/bcExample/basefeeExample.json#L164
    #[test]
    fn block_header_rlp_encoding_prague() {
        let expected_encoding = hex::decode("f90264a088867a8da7be57bcf8c17c2f19ddcd741aaca7236e21514efd3fce07f0e59c7da01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347942adc25665018aa1fe0e6bc666dac8fc2697ff9baa028b2411b8b56e872f6190379dcfcdcb73f835f67b0493e57b5587c53d3eeea50a091b7a6c2330ca44ce3895fd67915587a8900f8e807abec5ff5e299909d689162a0ccd78acb4b8076325dc580c8c1204c9361e2386a9aaaee95bb0acaa1c099fad0b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008001887fffffffffffffff830143a882079e42a000000000000000000000000000000000000000000000000000000000000200008800000000000000008403a699d0a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b4218080a00000000000000000000000000000000000000000000000000000000000000000a0e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855").unwrap();
        let expected_hash =
            B256::from_str("0xedaa151aff70c6ade40b90453841da58be2ca16a1c908874510601f8236e1c47")
                .unwrap();

        let header = Header {
            base_fee_per_gas: Some(0x03a699d0),
            blob_gas: Some(BlobGas {
                gas_used: 0x00u64,
                excess_gas: 0x00u64,
            }),
            logs_bloom: Bloom::ZERO,
            beneficiary: Address::from_str("0x2adc25665018aa1fe0e6bc666dac8fc2697ff9ba").unwrap(),
            difficulty: U256::ZERO,
            extra_data: hex::decode("42").unwrap().into(),
            gas_limit: 0x7fffffffffffffffu64,
            gas_used: 0x0143a8u64,
            mix_hash: B256::from_str(
                "0x0000000000000000000000000000000000000000000000000000000000020000",
            )
            .unwrap(),
            nonce: B64::ZERO,
            number: 0x01u64,
            parent_beacon_block_root: Some(B256::ZERO),
            parent_hash: B256::from_str(
                "0x88867a8da7be57bcf8c17c2f19ddcd741aaca7236e21514efd3fce07f0e59c7d",
            )
            .unwrap(),
            receipts_root: B256::from_str(
                "0xccd78acb4b8076325dc580c8c1204c9361e2386a9aaaee95bb0acaa1c099fad0",
            )
            .unwrap(),
            requests_hash: Some(
                B256::from_str(
                    "0xe3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                )
                .unwrap(),
            ),
            state_root: B256::from_str(
                "0x28b2411b8b56e872f6190379dcfcdcb73f835f67b0493e57b5587c53d3eeea50",
            )
            .unwrap(),
            timestamp: 0x079eu64,
            transactions_root: B256::from_str(
                "0x91b7a6c2330ca44ce3895fd67915587a8900f8e807abec5ff5e299909d689162",
            )
            .unwrap(),
            ommers_hash: KECCAK_RLP_EMPTY_ARRAY,
            withdrawals_root: Some(KECCAK_NULL_RLP),
        };

        let encoded = alloy_rlp::encode(&header);
        assert_eq!(encoded, expected_encoding);
        assert_eq!(header.hash(), expected_hash);
    }
}
