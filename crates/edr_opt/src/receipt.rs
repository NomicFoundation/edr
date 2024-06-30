use edr_eth::B256;

/// Typed receipt data for Optimism.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedData {
    /// Pre-EIP-658 legacy transaction receipt.
    PreEip658Legacy {
        /// State root
        state_root: B256,
    },
    /// Post-EIP-658 legacy transaction receipt.
    PostEip658Legacy {
        /// Status code
        status: u8,
    },
    /// EIP-2930 transaction receipt.
    Eip2930 {
        /// Status code
        status: u8,
    },
    /// EIP-1559 transaction receipt.
    Eip1559 {
        /// Status code
        status: u8,
    },
    /// EIP-4844 transaction receipt.
    Eip4844 {
        /// Status code
        status: u8,
    },
    /// Deposited transaction receipt.
    Deposited {
        /// Status code
        status: u8,
        /// The nonce used during execution.
        deposit_nonce: u64,
        /// The deposit receipt version.
        ///
        /// The deposit receipt version was introduced in Canyon to indicate an
        /// update to how receipt hashes should be computed when set.
        /// The state transition process ensures this is only set for
        /// post-Canyon deposit transactions.
        deposit_receipt_version: Option<u8>,
    },
}
