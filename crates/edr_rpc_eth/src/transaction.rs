use std::sync::OnceLock;

use edr_eth::{
    signature,
    transaction::{self, TxKind},
    AccessListItem, Address, Bytes, B256, U256,
};

/// RPC transaction
#[derive(Clone, Debug, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    /// hash of the transaction
    pub hash: B256,
    /// the number of transactions made by the sender prior to this one
    #[serde(with = "edr_eth::serde::u64")]
    pub nonce: u64,
    /// hash of the block where this transaction was in
    pub block_hash: Option<B256>,
    /// block number where this transaction was in
    pub block_number: Option<U256>,
    /// integer of the transactions index position in the block. null when its
    /// pending
    #[serde(with = "edr_eth::serde::optional_u64")]
    pub transaction_index: Option<u64>,
    /// address of the sender
    pub from: Address,
    /// address of the receiver. null when its a contract creation transaction.
    pub to: Option<Address>,
    /// value transferred in Wei
    pub value: U256,
    /// gas price provided by the sender in Wei
    pub gas_price: U256,
    /// gas provided by the sender
    pub gas: U256,
    /// the data sent along with the transaction
    pub input: Bytes,
    /// ECDSA recovery id
    #[serde(with = "edr_eth::serde::u64")]
    pub v: u64,
    /// Y-parity for EIP-2930 and EIP-1559 transactions. In theory these
    /// transactions types shouldn't have a `v` field, but in practice they
    /// are returned by nodes.
    #[serde(
        default,
        rename = "yParity",
        skip_serializing_if = "Option::is_none",
        with = "edr_eth::serde::optional_u64"
    )]
    pub y_parity: Option<u64>,
    /// ECDSA signature r
    pub r: U256,
    /// ECDSA signature s
    pub s: U256,
    /// chain ID
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "edr_eth::serde::optional_u64"
    )]
    pub chain_id: Option<u64>,
    /// integer of the transaction type, 0x0 for legacy transactions, 0x1 for
    /// access list types, 0x2 for dynamic fees
    #[serde(
        rename = "type",
        default,
        skip_serializing_if = "Option::is_none",
        with = "edr_eth::serde::optional_u64"
    )]
    pub transaction_type: Option<u64>,
    /// access list
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_list: Option<Vec<AccessListItem>>,
    /// max fee per gas
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fee_per_gas: Option<U256>,
    /// max priority fee per gas
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_priority_fee_per_gas: Option<U256>,
    /// The maximum total fee per gas the sender is willing to pay for blob gas
    /// in wei (EIP-4844)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fee_per_blob_gas: Option<U256>,
    /// List of versioned blob hashes associated with the transaction's EIP-4844
    /// data blobs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob_versioned_hashes: Option<Vec<B256>>,
}

impl Transaction {
    /// Returns whether the transaction has odd Y parity.
    pub fn odd_y_parity(&self) -> bool {
        self.v == 1 || self.v == 28
    }

    /// Returns whether the transaction is a legacy transaction.
    pub fn is_legacy(&self) -> bool {
        matches!(self.transaction_type, None | Some(0)) && matches!(self.v, 27 | 28)
    }
}

impl TryFrom<Transaction> for transaction::Signed {
    type Error = ConversionError;

    fn try_from(value: Transaction) -> Result<Self, Self::Error> {
        let kind = if let Some(to) = &value.to {
            TxKind::Call(*to)
        } else {
            TxKind::Create
        };

        let transaction = match value.transaction_type {
            Some(0) | None => {
                if value.is_legacy() {
                    transaction::Signed::PreEip155Legacy(transaction::signed::Legacy {
                        nonce: value.nonce,
                        gas_price: value.gas_price,
                        gas_limit: value.gas.to(),
                        kind,
                        value: value.value,
                        input: value.input,
                        // SAFETY: The `from` field represents the caller address of the signed
                        // transaction.
                        signature: unsafe {
                            signature::Fakeable::with_address_unchecked(
                                signature::SignatureWithRecoveryId {
                                    r: value.r,
                                    s: value.s,
                                    v: value.v,
                                },
                                value.from,
                            )
                        },
                        hash: OnceLock::from(value.hash),
                    })
                } else {
                    transaction::Signed::PostEip155Legacy(transaction::signed::Eip155 {
                        nonce: value.nonce,
                        gas_price: value.gas_price,
                        gas_limit: value.gas.to(),
                        kind,
                        value: value.value,
                        input: value.input,
                        // SAFETY: The `from` field represents the caller address of the signed
                        // transaction.
                        signature: unsafe {
                            signature::Fakeable::with_address_unchecked(
                                signature::SignatureWithRecoveryId {
                                    r: value.r,
                                    s: value.s,
                                    v: value.v,
                                },
                                value.from,
                            )
                        },
                        hash: OnceLock::from(value.hash),
                    })
                }
            }
            Some(1) => transaction::Signed::Eip2930(transaction::signed::Eip2930 {
                // SAFETY: The `from` field represents the caller address of the signed
                // transaction.
                signature: unsafe {
                    signature::Fakeable::with_address_unchecked(
                        signature::SignatureWithYParity {
                            y_parity: value.odd_y_parity(),
                            r: value.r,
                            s: value.s,
                        },
                        value.from,
                    )
                },
                chain_id: value.chain_id.ok_or(ConversionError::ChainId)?,
                nonce: value.nonce,
                gas_price: value.gas_price,
                gas_limit: value.gas.to(),
                kind,
                value: value.value,
                input: value.input,
                access_list: value.access_list.ok_or(ConversionError::AccessList)?.into(),
                hash: OnceLock::from(value.hash),
            }),
            Some(2) => transaction::Signed::Eip1559(transaction::signed::Eip1559 {
                // SAFETY: The `from` field represents the caller address of the signed
                // transaction.
                signature: unsafe {
                    signature::Fakeable::with_address_unchecked(
                        signature::SignatureWithYParity {
                            y_parity: value.odd_y_parity(),
                            r: value.r,
                            s: value.s,
                        },
                        value.from,
                    )
                },
                chain_id: value.chain_id.ok_or(ConversionError::ChainId)?,
                nonce: value.nonce,
                max_priority_fee_per_gas: value
                    .max_priority_fee_per_gas
                    .ok_or(ConversionError::MaxPriorityFeePerGas)?,
                max_fee_per_gas: value.max_fee_per_gas.ok_or(ConversionError::MaxFeePerGas)?,
                gas_limit: value.gas.to(),
                kind,
                value: value.value,
                input: value.input,
                access_list: value.access_list.ok_or(ConversionError::AccessList)?.into(),
                hash: OnceLock::from(value.hash),
            }),
            Some(3) => transaction::Signed::Eip4844(transaction::signed::Eip4844 {
                // SAFETY: The `from` field represents the caller address of the signed
                // transaction.
                signature: unsafe {
                    signature::Fakeable::with_address_unchecked(
                        signature::SignatureWithYParity {
                            r: value.r,
                            s: value.s,
                            y_parity: value.odd_y_parity(),
                        },
                        value.from,
                    )
                },
                chain_id: value.chain_id.ok_or(ConversionError::ChainId)?,
                nonce: value.nonce,
                max_priority_fee_per_gas: value
                    .max_priority_fee_per_gas
                    .ok_or(ConversionError::MaxPriorityFeePerGas)?,
                max_fee_per_gas: value.max_fee_per_gas.ok_or(ConversionError::MaxFeePerGas)?,
                max_fee_per_blob_gas: value
                    .max_fee_per_blob_gas
                    .ok_or(ConversionError::MaxFeePerBlobGas)?,
                gas_limit: value.gas.to(),
                to: value.to.ok_or(ConversionError::ReceiverAddress)?,
                value: value.value,
                input: value.input,
                access_list: value.access_list.ok_or(ConversionError::AccessList)?.into(),
                blob_hashes: value
                    .blob_versioned_hashes
                    .ok_or(ConversionError::BlobHashes)?,
                hash: OnceLock::from(value.hash),
            }),
            Some(r#type) => {
                log::warn!("Unsupported transaction type: {type}. Reverting to post-EIP 155 legacy transaction", );

                transaction::Signed::PostEip155Legacy(transaction::signed::Eip155 {
                    nonce: value.nonce,
                    gas_price: value.gas_price,
                    gas_limit: value.gas.to(),
                    kind,
                    value: value.value,
                    input: value.input,
                    // SAFETY: The `from` field represents the caller address of the signed
                    // transaction.
                    signature: unsafe {
                        signature::Fakeable::with_address_unchecked(
                            signature::SignatureWithRecoveryId {
                                r: value.r,
                                s: value.s,
                                v: value.v,
                            },
                            value.from,
                        )
                    },
                    hash: OnceLock::from(value.hash),
                })
            }
        };

        Ok(transaction)
    }
}

impl From<Transaction> for transaction::signed::Legacy {
    fn from(value: Transaction) -> Self {
        Self {
            nonce: value.nonce,
            gas_price: value.gas_price,
            gas_limit: value.gas.to(),
            kind: if let Some(to) = value.to {
                TxKind::Call(to)
            } else {
                TxKind::Create
            },
            value: value.value,
            input: value.input,
            // SAFETY: The `from` field represents the caller address of the signed
            // transaction.
            signature: unsafe {
                signature::Fakeable::with_address_unchecked(
                    signature::SignatureWithRecoveryId {
                        r: value.r,
                        s: value.s,
                        v: value.v,
                    },
                    value.from,
                )
            },
            hash: OnceLock::from(value.hash),
        }
    }
}

/// Error that occurs when trying to convert the JSON-RPC `Transaction` type.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    /// Missing access list
    #[error("Missing access list")]
    AccessList,
    /// EIP-4844 transaction is missing blob (versioned) hashes
    #[error("Missing blob hashes")]
    BlobHashes,
    /// Missing chain ID
    #[error("Missing chain ID")]
    ChainId,
    /// Missing max fee per gas
    #[error("Missing max fee per gas")]
    MaxFeePerGas,
    /// Missing max priority fee per gas
    #[error("Missing max priority fee per gas")]
    MaxPriorityFeePerGas,
    /// EIP-4844 transaction is missing the max fee per blob gas
    #[error("Missing max fee per blob gas")]
    MaxFeePerBlobGas,
    /// EIP-4844 transaction is missing the receiver (to) address
    #[error("Missing receiver (to) address")]
    ReceiverAddress,
}
