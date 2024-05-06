use edr_eth::{transaction::TxKind, Address, Bytes, B256, U256};

/// Deposited transaction.
///
/// For details, see <https://specs.optimism.io/protocol/deposits.html#the-deposited-transaction-type>.
#[cfg_attr(
    feature = "serde",
    derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)
)]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Transaction {
    /// Hash that uniquely identifies the origin of the deposit.
    pub source_hash: B256,
    /// The address of the sender account.
    pub from: Address,
    /// The address of the recipient account, or the null (zero-length) address
    /// if the deposited transaction is a contract creation.
    pub to: TxKind,
    /// The ETH value to mint on L2.
    pub mint: Option<u128>,
    ///  The ETH value to send to the recipient account.
    pub value: U256,
    /// The gas limit for the L2 transaction.
    #[cfg_attr(feature = "serde", serde(rename = "gas"))]
    pub gas_limit: u64,
    /// Field indicating if this transaction is exempt from the L2 gas limit.
    #[cfg_attr(feature = "serde", serde(rename = "isSystemTx"))]
    pub is_system_transaction: bool,
    #[cfg_attr(feature = "serde", serde(alias = "input"))]
    /// The calldata
    pub data: Bytes,
}
