mod difficulty;
mod overrides;

pub use alloy_eips::eip4895::Withdrawal;
use alloy_eips::eip7840::BlobParams;
use edr_chain_spec::{
    BlobExcessGasAndPrice, BlockEnvConstructor, BlockEnvForHardfork, BlockEnvTrait, EvmSpecId,
};
use edr_eip1559::BaseFeeParams;
pub use edr_eip4844::BlobGas;
use edr_eip7892::ScheduledBlobParams;
use edr_primitives::{b256, keccak256, Address, Bloom, Bytes, B256, B64, KECCAK_NULL_RLP, U256};
use edr_trie::ordered_trie_root;

pub use self::overrides::HeaderOverrides;
use crate::difficulty::calculate_ethash_canonical_difficulty;

/// ethereum block header
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    alloy_rlp::RlpDecodable,
    alloy_rlp::RlpEncodable,
    serde::Deserialize,
    serde::Serialize,
)]
#[serde(rename_all = "camelCase")]
#[rlp(trailing)]
pub struct BlockHeader {
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
    #[serde(with = "alloy_serde::quantity::opt")]
    pub base_fee_per_gas: Option<u128>,
    /// `WithdrawalsHash` was added by EIP-4895 and is ignored in legacy
    /// headers.
    pub withdrawals_root: Option<B256>,
    /// Blob gas was added by EIP-4844 and is ignored in older headers.
    #[serde(flatten)]
    pub blob_gas: Option<BlobGas>,
    /// The hash tree root of the parent beacon block for the given execution
    /// block (EIP-4788).
    pub parent_beacon_block_root: Option<B256>,
    /// The commitment hash calculated for a list of [EIP-7685] data requests.
    ///
    /// [EIP-7685]: https://eips.ethereum.org/EIPS/eip-7685
    pub requests_hash: Option<B256>,
}

impl BlockHeader {
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

pub fn blob_params_for_hardfork(
    evm_spec_id: EvmSpecId,
    timestamp: u64,
    scheduled_blob_params: Option<&ScheduledBlobParams>,
) -> BlobParams {
    if evm_spec_id >= EvmSpecId::OSAKA {
        if let Some(blob_param) = scheduled_blob_params
            .and_then(|params| params.active_scheduled_params_at_timestamp(timestamp))
        {
            *blob_param
        } else {
            BlobParams::osaka()
        }
    } else if evm_spec_id >= EvmSpecId::PRAGUE {
        BlobParams::prague()
    } else {
        BlobParams::cancun()
    }
}

/// Calculates the blob excess gas and price for the specified [`EvmSpecId`].
fn blob_excess_gas_and_price_for_evm_spec(
    blob_gas: &BlobGas,
    evm_spec_id: EvmSpecId,
    timestamp: u64,
    scheduled_blob_params: Option<&ScheduledBlobParams>,
) -> BlobExcessGasAndPrice {
    let blob_params = blob_params_for_hardfork(evm_spec_id, timestamp, scheduled_blob_params);

    BlobExcessGasAndPrice::new(
        blob_gas.excess_gas,
        blob_params
            .update_fraction
            .try_into()
            .expect("blob update fraction is too large"),
    )
}

impl<HardforkT: Into<EvmSpecId>> BlockEnvForHardfork<HardforkT> for BlockHeader {
    fn number_for_hardfork(&self, _hardfork: HardforkT) -> U256 {
        U256::from(self.number)
    }

    fn beneficiary_for_hardfork(&self, _hardfork: HardforkT) -> Address {
        self.beneficiary
    }

    fn timestamp_for_hardfork(&self, _hardfork: HardforkT) -> U256 {
        U256::from(self.timestamp)
    }

    fn gas_limit_for_hardfork(&self, _hardfork: HardforkT) -> u64 {
        self.gas_limit
    }

    fn basefee_for_hardfork(&self, _hardfork: HardforkT) -> u64 {
        self.base_fee_per_gas.map_or(0u64, |base_fee| {
            base_fee.try_into().expect("base fee is too large")
        })
    }

    fn difficulty_for_hardfork(&self, _hardfork: HardforkT) -> U256 {
        self.difficulty
    }

    fn prevrandao_for_hardfork(&self, hardfork: HardforkT) -> Option<B256> {
        if hardfork.into() >= EvmSpecId::MERGE {
            Some(self.mix_hash)
        } else {
            None
        }
    }

    fn blob_excess_gas_and_price_for_hardfork(
        &self,
        hardfork: HardforkT,
        scheduled_blob_params: Option<&ScheduledBlobParams>,
    ) -> Option<BlobExcessGasAndPrice> {
        self.blob_gas.as_ref().map(|blob_gas| {
            blob_excess_gas_and_price_for_evm_spec(
                blob_gas,
                hardfork.into(),
                self.timestamp,
                scheduled_blob_params,
            )
        })
    }
}

/// Wrapper type combining a header with its associated hardfork.
///
/// Both are needed to implement the [`BlockEnvTrait`] trait.
pub struct HeaderAndEvmSpec<'header, BlockHeaderT: BlockEnvForHardfork<HardforkT>, HardforkT> {
    pub hardfork: HardforkT,
    pub header: &'header BlockHeaderT,
    pub scheduled_blob_params: Option<ScheduledBlobParams>,
}

impl<'header, HardforkT, BlockHeaderT: BlockEnvForHardfork<HardforkT>>
    BlockEnvConstructor<HardforkT, &'header BlockHeaderT>
    for HeaderAndEvmSpec<'header, BlockHeaderT, HardforkT>
{
    fn new_block_env(
        header: &'header BlockHeaderT,
        hardfork: HardforkT,
        scheduled_blob_params: Option<ScheduledBlobParams>,
    ) -> HeaderAndEvmSpec<'header, BlockHeaderT, HardforkT> {
        HeaderAndEvmSpec {
            hardfork,
            header,
            scheduled_blob_params,
        }
    }
}

impl<HardforkT: Copy + Into<EvmSpecId>, BlockHeaderT: BlockEnvForHardfork<HardforkT>> BlockEnvTrait
    for HeaderAndEvmSpec<'_, BlockHeaderT, HardforkT>
{
    fn number(&self) -> U256 {
        self.header.number_for_hardfork(self.hardfork)
    }

    fn beneficiary(&self) -> Address {
        self.header.beneficiary_for_hardfork(self.hardfork)
    }

    fn timestamp(&self) -> U256 {
        self.header.timestamp_for_hardfork(self.hardfork)
    }

    fn gas_limit(&self) -> u64 {
        self.header.gas_limit_for_hardfork(self.hardfork)
    }

    fn basefee(&self) -> u64 {
        self.header.basefee_for_hardfork(self.hardfork)
    }

    fn difficulty(&self) -> U256 {
        self.header.difficulty_for_hardfork(self.hardfork)
    }

    fn prevrandao(&self) -> Option<B256> {
        self.header.prevrandao_for_hardfork(self.hardfork)
    }

    fn blob_excess_gas_and_price(&self) -> Option<BlobExcessGasAndPrice> {
        self.header.blob_excess_gas_and_price_for_hardfork(
            self.hardfork,
            self.scheduled_blob_params.as_ref(),
        )
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
    /// parent [`BlockHeader`] for the given [`EvmSpecId`].
    pub fn new<HardforkT: Clone + Into<EvmSpecId> + PartialOrd>(
        block_config: BlockConfig<HardforkT>,
        overrides: HeaderOverrides<HardforkT>,
        parent: Option<&BlockHeader>,
        ommers: &Vec<BlockHeader>,
        withdrawals: Option<&Vec<Withdrawal>>,
    ) -> Self {
        let BlockConfig {
            base_fee_params,
            hardfork,
            min_ethash_difficulty,
            scheduled_blob_params,
        } = block_config;

        let timestamp = overrides.timestamp.unwrap_or_default();
        let number = overridden_block_number(parent, &overrides);

        let parent_hash = overrides.parent_hash.unwrap_or_else(|| {
            if let Some(parent) = parent {
                parent.hash()
            } else {
                B256::ZERO
            }
        });

        let evm_spec_id = hardfork.clone().into();

        let base_fee = overrides.base_fee.or_else(|| {
            if evm_spec_id >= EvmSpecId::LONDON {
                Some(if let Some(parent) = &parent {
                    calculate_next_base_fee_per_gas(
                        parent,
                        overrides
                            .base_fee_params
                            .as_ref()
                            .unwrap_or(&base_fee_params),
                        hardfork,
                    )
                } else {
                    u128::from(alloy_eips::eip1559::INITIAL_BASE_FEE)
                })
            } else {
                None
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
                if evm_spec_id >= EvmSpecId::MERGE {
                    U256::ZERO
                } else if let Some(parent) = parent {
                    calculate_ethash_canonical_difficulty(
                        evm_spec_id,
                        parent,
                        number,
                        timestamp,
                        min_ethash_difficulty,
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
                if evm_spec_id >= EvmSpecId::MERGE {
                    B64::ZERO
                } else {
                    B64::from(66u64)
                }
            }),
            base_fee,
            withdrawals_root: overrides.withdrawals_root.or_else(|| {
                if evm_spec_id >= EvmSpecId::SHANGHAI {
                    let withdrawals_root = withdrawals.map_or(KECCAK_NULL_RLP, |withdrawals| {
                        ordered_trie_root(withdrawals.iter().map(alloy_rlp::encode))
                    });

                    Some(withdrawals_root)
                } else {
                    None
                }
            }),
            blob_gas: overrides.blob_gas.or_else(|| {
                if evm_spec_id >= EvmSpecId::CANCUN {
                    let excess_gas = parent.and_then(|parent| parent.blob_gas.as_ref()).map_or(
                        // For the first (post-fork) block, both parent.blob_gas_used and
                        // parent.excess_blob_gas are evaluated as 0.
                        0,
                        |BlobGas {
                             gas_used,
                             excess_gas,
                         }| {
                            let blob_params = blob_params_for_hardfork(
                                evm_spec_id,
                                timestamp,
                                scheduled_blob_params.as_ref(),
                            );

                            let base_fee = if evm_spec_id >= EvmSpecId::OSAKA {
                                base_fee.expect("base fee must be set for post-Osaka blocks")
                            } else {
                                // In pre-Osaka (EIP-4844) scenarios, the base fee parameter is not
                                // used in excess blob gas calculation. Passing 0 is acceptable here
                                // because `next_block_excess_blob_gas_osaka` ignores the base fee
                                // value for these hardforks.
                                0
                            };

                            blob_params.next_block_excess_blob_gas_osaka(
                                *excess_gas,
                                *gas_used,
                                base_fee.try_into().expect("base fee is too large"),
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
                if evm_spec_id >= EvmSpecId::CANCUN {
                    // Initial value from https://eips.ethereum.org/EIPS/eip-4788
                    Some(B256::ZERO)
                } else {
                    None
                }
            }),
            requests_hash: overrides.requests_hash.or_else(|| {
                if evm_spec_id >= EvmSpecId::PRAGUE {
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

impl From<BlockHeader> for PartialHeader {
    fn from(header: BlockHeader) -> PartialHeader {
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

impl<HardforkT: Into<EvmSpecId>> BlockEnvForHardfork<HardforkT> for PartialHeader {
    fn number_for_hardfork(&self, _hardfork: HardforkT) -> U256 {
        U256::from(self.number)
    }

    fn beneficiary_for_hardfork(&self, _hardfork: HardforkT) -> Address {
        self.beneficiary
    }

    fn timestamp_for_hardfork(&self, _hardfork: HardforkT) -> U256 {
        U256::from(self.timestamp)
    }

    fn gas_limit_for_hardfork(&self, _hardfork: HardforkT) -> u64 {
        self.gas_limit
    }

    fn basefee_for_hardfork(&self, _hardfork: HardforkT) -> u64 {
        self.base_fee.map_or(0u64, |base_fee| {
            base_fee.try_into().expect("base fee is too large")
        })
    }

    fn difficulty_for_hardfork(&self, _hardfork: HardforkT) -> U256 {
        self.difficulty
    }

    fn prevrandao_for_hardfork(&self, hardfork: HardforkT) -> Option<B256> {
        if hardfork.into() >= EvmSpecId::MERGE {
            Some(self.mix_hash)
        } else {
            None
        }
    }

    fn blob_excess_gas_and_price_for_hardfork(
        &self,
        hardfork: HardforkT,
        scheduled_blob_params: Option<&ScheduledBlobParams>,
    ) -> Option<BlobExcessGasAndPrice> {
        self.blob_gas.as_ref().map(|blob_gas| {
            blob_excess_gas_and_price_for_evm_spec(
                blob_gas,
                hardfork.into(),
                self.timestamp,
                scheduled_blob_params,
            )
        })
    }
}

/// Defines the configurations needed for building a block
#[derive(Clone, Debug)]
pub struct BlockConfig<HardforkT> {
    /// Associated base fee params
    pub base_fee_params: BaseFeeParams<HardforkT>,
    /// Associated hardfork
    pub hardfork: HardforkT,
    /// Associated minimum ethash difficulty
    pub min_ethash_difficulty: u64,
    /// Scheduled blob parameter only hardfork parameters
    pub scheduled_blob_params: Option<ScheduledBlobParams>,
}

/// Determines the block number based on the provided parent header and
/// (potential) overrides.
pub fn overridden_block_number<HardforkT>(
    parent_header: Option<&BlockHeader>,
    overrides: &HeaderOverrides<HardforkT>,
) -> u64 {
    overrides.number.unwrap_or({
        if let Some(parent) = parent_header {
            parent.number + 1
        } else {
            0
        }
    })
}

/// Calculates the next base fee for a post-London block, given the parent's
/// header.
///
/// # Panics
///
/// Panics if the parent header does not contain a base fee.
pub fn calculate_next_base_fee_per_gas<HardforkT: PartialOrd>(
    parent: &BlockHeader,
    base_fee_params: &BaseFeeParams<HardforkT>,
    hardfork: HardforkT,
) -> u128 {
    let base_fee_params = base_fee_params
        .at_condition(hardfork, parent.number + 1)
        .copied()
        .expect("Chain must have base fee params for post-London hardforks");

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
pub fn calculate_next_base_fee_per_blob_gas<HardforkT: Into<EvmSpecId>>(
    parent: &BlockHeader,
    hardfork: HardforkT,
    scheduled_blob_params: Option<&ScheduledBlobParams>,
) -> u128 {
    let evm_spec_id = hardfork.into();

    parent
        .blob_gas
        .as_ref()
        .map_or(0u128, |BlobGas { excess_gas, .. }| {
            let blob_params =
                blob_params_for_hardfork(evm_spec_id, parent.timestamp, scheduled_blob_params);

            blob_params.calc_blob_fee(*excess_gas)
        })
}

#[cfg(test)]
mod tests {
    use std::{
        str::FromStr,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use alloy_rlp::Decodable as _;
    use edr_primitives::{hex, KECCAK_RLP_EMPTY_ARRAY};

    use super::*;

    #[test]
    fn header_rlp_roundtrip() {
        let mut header = BlockHeader {
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
        let decoded = BlockHeader::decode(&mut encoded.as_slice()).unwrap();
        assert_eq!(header, decoded);

        header.base_fee_per_gas = Some(12345);

        let encoded = alloy_rlp::encode(&header);
        let decoded = BlockHeader::decode(&mut encoded.as_slice()).unwrap();
        assert_eq!(header, decoded);
    }

    #[test]
    // Test vector from: https://eips.ethereum.org/EIPS/eip-2481
    fn test_encode_block_header() {
        let expected = hex::decode("f901f9a00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000940000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000b90100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008208ae820d0582115c8215b3821a0a827788a00000000000000000000000000000000000000000000000000000000000000000880000000000000000").unwrap();

        let header = BlockHeader {
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
        let header = BlockHeader {
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

        let expected = BlockHeader {
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
        let decoded = BlockHeader::decode(&mut data.as_slice()).unwrap();
        assert_eq!(decoded, expected);
    }

    // Test vector from https://github.com/ethereum/tests/blob/a33949df17a1c382ffee5666e66d26bde7a089f9/EIPTests/Pyspecs/cancun/eip4844_blobs/correct_increasing_blob_gas_costs.json#L16
    #[test]
    fn block_header_rlp_encoding_cancun() {
        let expected_encoding = hex::decode("f90242a0258811d02512e87e09253a948330eff05da06b7656143a211fa3687901217f57a01dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347942adc25665018aa1fe0e6bc666dac8fc2697ff9baa06a086c92bb1d4ee6dc4ca73e66529037591bd4d6590350f6c904bc78dc21b75ca0dc387fc6ef9e3eb53baa85df89a1f9b91a4a9ab472ee7e928b4b7fdc06dfa5d1a0eaa8c40899a61ae59615cf9985f5e2194f8fd2b57d273be63bde6733e89b12abb9010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000800188016345785d8a00008252080c80a0000000000000000000000000000000000000000000000000000000000000000088000000000000000007a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b4218308000083220000a00000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let expected_hash =
            B256::from_str("0xd2caf87ef0ecbbf1d8721e4f63d56b3a5b4bf8b5faa0409aa6b99a729affe346")
                .unwrap();

        let header = BlockHeader {
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

        let header = BlockHeader {
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

    fn to_timestamp(time: SystemTime) -> u64 {
        time.duration_since(UNIX_EPOCH).unwrap().as_secs()
    }
    const ONE_HOUR: Duration = Duration::from_secs(60 * 60);
    const ONE_DAY: Duration = Duration::from_secs(60 * 6 * 24);

    #[test]
    fn test_blob_params_for_hardfork_should_not_return_bpo_values_before_osaka() {
        let now = to_timestamp(SystemTime::now());
        let an_hour_ago = to_timestamp(SystemTime::now().checked_sub(ONE_HOUR).unwrap());
        let scheduled_blob_params: ScheduledBlobParams =
            vec![(an_hour_ago, BlobParams::bpo1())].into();
        let blob_params =
            blob_params_for_hardfork(EvmSpecId::PRAGUE, now, Some(&scheduled_blob_params));
        assert_eq!(blob_params, BlobParams::prague());
    }

    #[test]
    fn test_blob_params_for_hardfork_should_not_return_bpo_values_after_osaka_if_not_activated_yet()
    {
        let now = to_timestamp(SystemTime::now());
        let in_one_hour = to_timestamp(SystemTime::now().checked_add(ONE_HOUR).unwrap());
        let scheduled_blob_params: ScheduledBlobParams =
            vec![(in_one_hour, BlobParams::bpo1())].into();
        let blob_params =
            blob_params_for_hardfork(EvmSpecId::OSAKA, now, Some(&scheduled_blob_params));
        assert_eq!(blob_params, BlobParams::osaka());
    }

    #[test]
    fn test_blob_params_for_hardfork_should_return_bpo_values_after_osaka_if_activated() {
        let now = to_timestamp(SystemTime::now());
        let an_hour_ago = to_timestamp(SystemTime::now().checked_sub(ONE_HOUR).unwrap());
        let scheduled_blob_params: ScheduledBlobParams =
            vec![(an_hour_ago, BlobParams::bpo1())].into();
        let blob_params =
            blob_params_for_hardfork(EvmSpecId::OSAKA, now, Some(&scheduled_blob_params));
        assert_eq!(blob_params, BlobParams::bpo1());
    }

    #[test]
    fn test_blob_params_for_hardfork_should_return_more_recent_activated_param() {
        let now = to_timestamp(SystemTime::now());
        let an_hour_ago = to_timestamp(SystemTime::now().checked_sub(ONE_HOUR).unwrap());
        let a_day_ago = to_timestamp(SystemTime::now().checked_sub(ONE_DAY).unwrap());
        let scheduled_blob_params: ScheduledBlobParams = vec![
            (an_hour_ago, BlobParams::bpo2()),
            (a_day_ago, BlobParams::bpo1()),
        ]
        .into();
        let blob_params =
            blob_params_for_hardfork(EvmSpecId::OSAKA, now, Some(&scheduled_blob_params));
        assert_eq!(blob_params, BlobParams::bpo2());
    }
}
