use super::BlobGas;
use crate::{eips::eip1559::ConstantBaseFeeParams, Address, Bytes, B256, B64, U256};

/// Data of a block header
#[derive(Debug, Default)]
pub struct HeaderOverrides {
    /// The parent block's hash
    pub parent_hash: Option<B256>,
    /// The ommers' root hash
    pub ommers_hash: B256,
    /// The block's beneficiary
    pub beneficiary: Option<Address>,
    /// The state's root hash
    pub state_root: Option<B256>,
    /// The block's difficulty
    pub difficulty: Option<U256>,
    /// The block's number
    pub number: Option<u64>,
    /// The block's gas limit
    pub gas_limit: Option<u64>,
    /// The block's timestamp
    pub timestamp: Option<u64>,
    /// The block's extra data
    pub extra_data: Option<Bytes>,
    /// The block's mix hash
    pub mix_hash: Option<B256>,
    /// The block's nonce
    pub nonce: Option<B64>,
    /// The block's base gas fee
    pub base_fee: Option<u128>,
    /// The parameters for calculating the base fee, used in EIP-1559.
    ///
    /// These only override the default base fee parameters if
    /// [`BlockOptions::base_fee`] is not set.
    pub base_fee_params: Option<ConstantBaseFeeParams>,
    /// The block's withdrawals root, which is the hash tree root of the
    /// withdrawals trie.
    ///
    /// This will override the hash tree root of [`BlockOptions::withdrawals`].
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
