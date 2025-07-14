/// Types for transaction gossip (aka pooled transactions)
pub mod pooled;
/// Types for transaction requests.
pub mod request;
/// Types for signed transactions.
pub mod signed;
/// The L1 transaction type.
pub mod r#type;

use ambassador::delegatable_trait_remote;

/// Convenience type alias for [`self::pooled::L1PooledTransaction`].
///
/// This allows usage like [`edr_chain_l1::Pooled`].
pub type Pooled = self::pooled::L1PooledTransaction;

/// Convenience type alias for [`self::request::L1TransactionRequest`].
///
/// This allows usage like [`edr_chain_l1::Request`].
pub type Request = self::request::L1TransactionRequest;

/// Convenience type alias for [`self::signed::L1SignedTransaction`].
///
/// This allows usage like [`edr_chain_l1::Signed`].
pub type Signed = self::signed::L1SignedTransaction;

/// Convenience type alias for [`self::r#type::L1TransactionType`].
///
/// This allows usage like [`edr_chain_l1::Type`].
pub type Type = self::r#type::L1TransactionType;

#[delegatable_trait_remote]
trait ExecutableTransaction {
    fn caller(&self) -> &Address;

    fn gas_limit(&self) -> u64;

    fn gas_price(&self) -> &u128;

    fn kind(&self) -> TxKind;

    fn value(&self) -> &U256;

    fn data(&self) -> &Bytes;

    fn nonce(&self) -> u64;

    fn chain_id(&self) -> Option<u64>;

    fn access_list(&self) -> Option<&[edr_eth::eips::eip2930::AccessListItem]>;

    fn effective_gas_price(&self, block_base_fee: u128) -> Option<u128>;

    fn max_fee_per_gas(&self) -> Option<&u128>;

    fn max_priority_fee_per_gas(&self) -> Option<&u128>;

    fn blob_hashes(&self) -> &[B256];

    fn max_fee_per_blob_gas(&self) -> Option<&u128>;

    fn total_blob_gas(&self) -> Option<u64>;

    fn authorization_list(&self) -> Option<&[edr_eth::eips::eip7702::SignedAuthorization]>;

    fn rlp_encoding(&self) -> &Bytes;

    fn transaction_hash(&self) -> &B256;
}
