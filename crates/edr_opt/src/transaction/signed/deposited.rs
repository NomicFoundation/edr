use edr_eth::B256;
use revm::primitives::keccak256;

use super::Deposited;

impl Deposited {
    /// Returns the transaction's hash.
    pub fn transaction_hash(&self) -> &B256 {
        self.hash.get_or_init(|| keccak256(alloy_rlp::encode(self)))
    }
}

impl PartialEq for Deposited {
    fn eq(&self, other: &Self) -> bool {
        // Custom implementation of `PartialEq` to ignore the `hash` field.
        self.source_hash == other.source_hash
            && self.from == other.from
            && self.to == other.to
            && self.mint == other.mint
            && self.value == other.value
            && self.gas_limit == other.gas_limit
            && self.is_system_transaction == other.is_system_transaction
            && self.data == other.data
    }
}
