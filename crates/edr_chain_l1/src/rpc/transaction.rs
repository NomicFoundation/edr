//! L1 Ethereum JSON-RPC transaction types
mod request;

use std::{ops::Deref, sync::OnceLock};

use edr_block_api::Block;
use edr_block_header::BlockHeader;
use edr_chain_spec::{EvmSpecId, ExecutableTransaction};
use edr_chain_spec_rpc::{RpcTransaction, RpcTypeFrom};
use edr_primitives::{Address, Bytes, B256, U256};
use edr_signer::{
    FakeableSignature, SignatureWithRecoveryId, SignatureWithYParity, SignatureWithYParityArgs,
};
use edr_transaction::{
    BlockDataForTransaction, IsEip4844, IsLegacy, SignedTransaction as _, TransactionAndBlock,
    TransactionType, TxKind,
};

pub use self::request::L1RpcTransactionRequest;
use crate::{Hardfork, L1SignedTransaction, L1TransactionType};

pub type Request = L1RpcTransactionRequest;

/// RPC transaction
#[derive(Clone, Debug, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1RpcTransaction {
    /// hash of the transaction
    pub hash: B256,
    /// the number of transactions made by the sender prior to this one
    #[serde(with = "alloy_serde::quantity")]
    pub nonce: u64,
    /// hash of the block where this transaction was in
    pub block_hash: Option<B256>,
    /// block number where this transaction was in
    #[serde(with = "alloy_serde::quantity::opt")]
    pub block_number: Option<u64>,
    /// integer of the transactions index position in the block. null when its
    /// pending
    #[serde(with = "alloy_serde::quantity::opt")]
    pub transaction_index: Option<u64>,
    /// address of the sender
    pub from: Address,
    /// address of the receiver. null when its a contract creation transaction.
    pub to: Option<Address>,
    /// value transferred in Wei
    pub value: U256,
    /// gas price provided by the sender in Wei
    #[serde(with = "alloy_serde::quantity")]
    pub gas_price: u128,
    /// gas provided by the sender
    pub gas: U256,
    /// the data sent along with the transaction
    pub input: Bytes,
    /// chain ID
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "alloy_serde::quantity::opt"
    )]
    pub chain_id: Option<u64>,
    /// integer of the transaction type, 0x0 for legacy transactions, 0x1 for
    /// access list types, 0x2 for dynamic fees
    #[serde(
        rename = "type",
        default,
        skip_serializing_if = "Option::is_none",
        with = "alloy_serde::quantity::opt"
    )]
    pub transaction_type: Option<u8>,
    /// access list
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_list: Option<Vec<edr_eip2930::AccessListItem>>,
    /// max fee per gas
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "alloy_serde::quantity::opt"
    )]
    pub max_fee_per_gas: Option<u128>,
    /// max priority fee per gas
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "alloy_serde::quantity::opt"
    )]
    pub max_priority_fee_per_gas: Option<u128>,
    /// The maximum total fee per gas the sender is willing to pay for blob gas
    /// in wei (EIP-4844)
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "alloy_serde::quantity::opt"
    )]
    pub max_fee_per_blob_gas: Option<u128>,
    /// List of versioned blob hashes associated with the transaction's EIP-4844
    /// data blobs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob_versioned_hashes: Option<Vec<B256>>,
    /// Authorizations are used to temporarily set the code of its signer to
    /// the code referenced by `address`. These also include a `chain_id` (which
    /// can be set to zero and not evaluated) as well as an optional `nonce`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization_list: Option<Vec<edr_eip7702::SignedAuthorization>>,
}

impl L1RpcTransaction {
    pub fn new(
        transaction: &(impl ExecutableTransaction + TransactionType + IsEip4844 + IsLegacy),
        header: Option<&BlockHeader>,
        transaction_index: Option<u64>,
        is_pending: bool,
        hardfork: EvmSpecId,
    ) -> Self {
        let base_fee = header.and_then(|header| header.base_fee_per_gas);
        let gas_price = if let Some(base_fee) = base_fee {
            transaction
                .effective_gas_price(base_fee)
                .unwrap_or_else(|| *transaction.gas_price())
        } else {
            // We are following Hardhat's behavior of returning the max fee per gas for
            // pending transactions.
            *transaction.gas_price()
        };

        let chain_id = transaction.chain_id().and_then(|chain_id| {
            // Following Hardhat in not returning `chain_id` for `PostEip155Legacy` legacy
            // transactions even though the chain id would be recoverable.
            if transaction.is_legacy() {
                None
            } else {
                Some(chain_id)
            }
        });

        let show_transaction_type = hardfork >= EvmSpecId::BERLIN;
        let is_typed_transaction = !transaction.is_legacy();
        let transaction_type = if show_transaction_type || is_typed_transaction {
            Some(transaction.transaction_type())
        } else {
            None
        };

        let (block_hash, block_number) = if is_pending {
            (None, None)
        } else {
            header.map(|header| (header.hash(), header.number)).unzip()
        };

        let transaction_index = if is_pending { None } else { transaction_index };

        let access_list = transaction
            .access_list()
            .map(<[edr_eip2930::AccessListItem]>::to_vec);

        let blob_versioned_hashes = if transaction.is_eip4844() {
            Some(transaction.blob_hashes().to_vec())
        } else {
            None
        };

        Self {
            hash: *transaction.transaction_hash(),
            nonce: transaction.nonce(),
            block_hash,
            block_number,
            transaction_index,
            from: *transaction.caller(),
            to: transaction.kind().to().copied(),
            value: *transaction.value(),
            gas_price,
            gas: U256::from(transaction.gas_limit()),
            input: transaction.data().clone(),
            chain_id,
            transaction_type: transaction_type.map(Into::<u8>::into),
            access_list,
            max_fee_per_gas: transaction.max_fee_per_gas().copied(),
            max_priority_fee_per_gas: transaction.max_priority_fee_per_gas().cloned(),
            max_fee_per_blob_gas: transaction.max_fee_per_blob_gas().cloned(),
            blob_versioned_hashes,
            authorization_list: transaction
                .authorization_list()
                .map(<[edr_eip7702::SignedAuthorization]>::to_vec),
        }
    }
}

/// RPC transaction with signature.
#[derive(Clone, Debug, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct L1RpcTransactionWithSignature {
    /// Transaction
    #[serde(flatten)]
    transaction: L1RpcTransaction,
    /// ECDSA recovery id
    #[serde(with = "alloy_serde::quantity")]
    pub v: u64,
    /// Y-parity for EIP-2930 and EIP-1559 transactions. In theory these
    /// transactions types shouldn't have a `v` field, but in practice they
    /// are returned by nodes.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "alloy_serde::quantity::opt"
    )]
    pub y_parity: Option<bool>,
    /// ECDSA signature r
    pub r: U256,
    /// ECDSA signature s
    pub s: U256,
}

impl L1RpcTransactionWithSignature {
    /// Creates a new instance from an RPC transaction and signature.
    pub fn new(
        transaction: L1RpcTransaction,
        r: U256,
        s: U256,
        v: u64,
        _y_parity: Option<bool>,
    ) -> Self {
        Self {
            transaction,
            v,
            // Following Hardhat in always returning `v` instead of `y_parity`.
            y_parity: None,
            r,
            s,
        }
    }

    /// Returns whether the transaction has odd Y parity.
    pub fn odd_y_parity(&self) -> bool {
        self.v == 1 || self.v == 28
    }

    /// Returns whether the transaction is a legacy transaction.
    pub fn is_legacy(&self) -> bool {
        matches!(self.transaction_type, None | Some(0)) && matches!(self.v, 27 | 28)
    }
}

impl Deref for L1RpcTransactionWithSignature {
    type Target = L1RpcTransaction;

    fn deref(&self) -> &Self::Target {
        &self.transaction
    }
}

impl From<L1RpcTransactionWithSignature> for edr_transaction::signed::Legacy {
    fn from(value: L1RpcTransactionWithSignature) -> Self {
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
            input: value.transaction.input,
            // SAFETY: The `from` field represents the caller address of the signed
            // transaction.
            signature: unsafe {
                FakeableSignature::with_address_unchecked(
                    SignatureWithRecoveryId {
                        r: value.r,
                        s: value.s,
                        v: value.v,
                    },
                    value.transaction.from,
                )
            },
            hash: OnceLock::from(value.transaction.hash),
            rlp_encoding: OnceLock::new(),
        }
    }
}

impl From<L1RpcTransactionWithSignature> for edr_transaction::signed::Eip155 {
    fn from(value: L1RpcTransactionWithSignature) -> Self {
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
            input: value.transaction.input,
            // SAFETY: The `from` field represents the caller address of the signed
            // transaction.
            signature: unsafe {
                FakeableSignature::with_address_unchecked(
                    SignatureWithRecoveryId {
                        r: value.r,
                        s: value.s,
                        v: value.v,
                    },
                    value.transaction.from,
                )
            },
            hash: OnceLock::from(value.transaction.hash),
            rlp_encoding: OnceLock::new(),
        }
    }
}

impl RpcTransaction for L1RpcTransactionWithSignature {
    fn block_hash(&self) -> Option<&B256> {
        self.block_hash.as_ref()
    }
}

impl<BlockT: Block<L1SignedTransaction>>
    RpcTypeFrom<TransactionAndBlock<BlockT, L1SignedTransaction>>
    for L1RpcTransactionWithSignature
{
    type Hardfork = Hardfork;

    fn rpc_type_from(
        value: &TransactionAndBlock<BlockT, L1SignedTransaction>,
        hardfork: Self::Hardfork,
    ) -> Self {
        let (header, transaction_index) = value
            .block_data
            .as_ref()
            .map(
                |BlockDataForTransaction {
                     block,
                     transaction_index,
                 }| (block.block_header(), *transaction_index),
            )
            .unzip();

        let transaction = L1RpcTransaction::new(
            &value.transaction,
            header,
            transaction_index,
            value.is_pending,
            hardfork,
        );
        let signature = value.transaction.signature();

        L1RpcTransactionWithSignature::new(
            transaction,
            signature.r(),
            signature.s(),
            signature.v(),
            signature.y_parity(),
        )
    }
}

impl TryFrom<L1RpcTransactionWithSignature> for edr_transaction::signed::Eip2930 {
    type Error = RpcTransactionConversionError;

    fn try_from(value: L1RpcTransactionWithSignature) -> Result<Self, Self::Error> {
        let transaction = Self {
            // SAFETY: The `from` field represents the caller address of the signed
            // transaction.
            signature: unsafe {
                FakeableSignature::with_address_unchecked(
                    SignatureWithYParity::new(SignatureWithYParityArgs {
                        r: value.r,
                        s: value.s,
                        y_parity: value.odd_y_parity(),
                    }),
                    value.from,
                )
            },
            chain_id: value
                .chain_id
                .ok_or(RpcTransactionConversionError::ChainId)?,
            nonce: value.nonce,
            gas_price: value.gas_price,
            gas_limit: value.gas.to(),
            kind: if let Some(to) = value.to {
                TxKind::Call(to)
            } else {
                TxKind::Create
            },
            value: value.value,
            input: value.transaction.input,
            access_list: value
                .transaction
                .access_list
                .ok_or(RpcTransactionConversionError::AccessList)?
                .into(),
            hash: OnceLock::from(value.transaction.hash),
            rlp_encoding: OnceLock::new(),
        };

        Ok(transaction)
    }
}

impl TryFrom<L1RpcTransactionWithSignature> for edr_transaction::signed::Eip1559 {
    type Error = RpcTransactionConversionError;

    fn try_from(value: L1RpcTransactionWithSignature) -> Result<Self, Self::Error> {
        let transaction = Self {
            // SAFETY: The `from` field represents the caller address of the signed
            // transaction.
            signature: unsafe {
                FakeableSignature::with_address_unchecked(
                    SignatureWithYParity::new(SignatureWithYParityArgs {
                        r: value.r,
                        s: value.s,
                        y_parity: value.odd_y_parity(),
                    }),
                    value.from,
                )
            },
            chain_id: value
                .chain_id
                .ok_or(RpcTransactionConversionError::ChainId)?,
            nonce: value.nonce,
            max_priority_fee_per_gas: value
                .max_priority_fee_per_gas
                .ok_or(RpcTransactionConversionError::MaxPriorityFeePerGas)?,
            max_fee_per_gas: value
                .max_fee_per_gas
                .ok_or(RpcTransactionConversionError::MaxFeePerGas)?,
            gas_limit: value.gas.to(),
            kind: if let Some(to) = value.to {
                TxKind::Call(to)
            } else {
                TxKind::Create
            },
            value: value.value,
            input: value.transaction.input,
            access_list: value
                .transaction
                .access_list
                .ok_or(RpcTransactionConversionError::AccessList)?
                .into(),
            hash: OnceLock::from(value.transaction.hash),
            rlp_encoding: OnceLock::new(),
        };

        Ok(transaction)
    }
}

impl TryFrom<L1RpcTransactionWithSignature> for edr_transaction::signed::Eip4844 {
    type Error = RpcTransactionConversionError;

    fn try_from(value: L1RpcTransactionWithSignature) -> Result<Self, Self::Error> {
        let transaction = Self {
            // SAFETY: The `from` field represents the caller address of the signed
            // transaction.
            signature: unsafe {
                FakeableSignature::with_address_unchecked(
                    SignatureWithYParity::new(SignatureWithYParityArgs {
                        r: value.r,
                        s: value.s,
                        y_parity: value.odd_y_parity(),
                    }),
                    value.from,
                )
            },
            chain_id: value
                .chain_id
                .ok_or(RpcTransactionConversionError::ChainId)?,
            nonce: value.nonce,
            max_priority_fee_per_gas: value
                .max_priority_fee_per_gas
                .ok_or(RpcTransactionConversionError::MaxPriorityFeePerGas)?,
            max_fee_per_gas: value
                .max_fee_per_gas
                .ok_or(RpcTransactionConversionError::MaxFeePerGas)?,
            max_fee_per_blob_gas: value
                .max_fee_per_blob_gas
                .ok_or(RpcTransactionConversionError::MaxFeePerBlobGas)?,
            gas_limit: value.gas.to(),
            to: value
                .to
                .ok_or(RpcTransactionConversionError::ReceiverAddress)?,
            value: value.value,
            input: value.transaction.input,
            access_list: value
                .transaction
                .access_list
                .ok_or(RpcTransactionConversionError::AccessList)?
                .into(),
            blob_hashes: value
                .transaction
                .blob_versioned_hashes
                .ok_or(RpcTransactionConversionError::BlobHashes)?,
            hash: OnceLock::from(value.transaction.hash),
            rlp_encoding: OnceLock::new(),
        };

        Ok(transaction)
    }
}

impl TryFrom<L1RpcTransactionWithSignature> for edr_transaction::signed::Eip7702 {
    type Error = RpcTransactionConversionError;

    fn try_from(value: L1RpcTransactionWithSignature) -> Result<Self, Self::Error> {
        let transaction = Self {
            // SAFETY: The `from` field represents the caller address of the signed
            // transaction.
            signature: unsafe {
                FakeableSignature::with_address_unchecked(
                    SignatureWithYParity::new(SignatureWithYParityArgs {
                        r: value.r,
                        s: value.s,
                        y_parity: value.odd_y_parity(),
                    }),
                    value.from,
                )
            },
            chain_id: value
                .chain_id
                .ok_or(RpcTransactionConversionError::ChainId)?,
            nonce: value.nonce,
            max_priority_fee_per_gas: value
                .max_priority_fee_per_gas
                .ok_or(RpcTransactionConversionError::MaxPriorityFeePerGas)?,
            max_fee_per_gas: value
                .max_fee_per_gas
                .ok_or(RpcTransactionConversionError::MaxFeePerGas)?,
            gas_limit: value.gas.to(),
            to: value
                .to
                .ok_or(RpcTransactionConversionError::ReceiverAddress)?,
            value: value.value,
            input: value.transaction.input,
            access_list: value
                .transaction
                .access_list
                .ok_or(RpcTransactionConversionError::AccessList)?
                .into(),
            authorization_list: value
                .transaction
                .authorization_list
                .ok_or(RpcTransactionConversionError::AuthorizationList)?,
            hash: OnceLock::from(value.transaction.hash),
            rlp_encoding: OnceLock::new(),
        };

        Ok(transaction)
    }
}

impl TryFrom<L1RpcTransactionWithSignature> for L1SignedTransaction {
    type Error = RpcTransactionConversionError;

    fn try_from(value: L1RpcTransactionWithSignature) -> Result<Self, Self::Error> {
        let transaction_type = match value
            .transaction_type
            .map_or(Ok(L1TransactionType::Legacy), L1TransactionType::try_from)
        {
            Ok(r#type) => r#type,
            Err(r#type) => {
                log::warn!(
                    "Unsupported transaction type: {type}. Reverting to post-EIP 155 legacy transaction"
                );

                // As the transaction type is not 0 or `None`, this will always result in a
                // post-EIP 155 legacy transaction.
                L1TransactionType::Legacy
            }
        };

        let transaction = match transaction_type {
            L1TransactionType::Legacy => {
                if value.is_legacy() {
                    Self::PreEip155Legacy(value.into())
                } else {
                    Self::PostEip155Legacy(value.into())
                }
            }
            L1TransactionType::Eip2930 => Self::Eip2930(value.try_into()?),
            L1TransactionType::Eip1559 => Self::Eip1559(value.try_into()?),
            L1TransactionType::Eip4844 => Self::Eip4844(value.try_into()?),
            L1TransactionType::Eip7702 => Self::Eip7702(value.try_into()?),
        };

        Ok(transaction)
    }
}

/// Error that occurs when trying to convert the JSON-RPC `Transaction` type.
#[derive(Debug, thiserror::Error)]
pub enum RpcTransactionConversionError {
    /// Missing access list
    #[error("Missing access list")]
    AccessList,
    /// Missing authorization list
    #[error("Missing authorization list")]
    AuthorizationList,
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
    /// EIP-4844 or EIP-7702 transaction is missing the receiver (to) address
    #[error("Missing receiver (to) address")]
    ReceiverAddress,
}
