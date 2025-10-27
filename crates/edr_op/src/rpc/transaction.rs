use std::sync::OnceLock;

use edr_block_api::Block;
use edr_chain_l1::rpc::transaction::{L1RpcTransaction, L1RpcTransactionWithSignature};
use edr_primitives::B256;
use edr_rpc_spec::{RpcTransaction, RpcTypeFrom};
use edr_signer::Signature;
use edr_transaction::{
    BlockDataForTransaction, MaybeSignedTransaction as _, TransactionAndBlock, TxKind,
};

use super::Transaction;
use crate::{
    transaction::{self, OpSignedTransaction, OpTxTrait as _},
    Hardfork,
};

impl RpcTransaction for Transaction {
    fn block_hash(&self) -> Option<&B256> {
        self.l1.block_hash.as_ref()
    }
}

impl TryFrom<Transaction> for transaction::signed::Deposit {
    type Error = ConversionError;

    fn try_from(value: Transaction) -> Result<Self, Self::Error> {
        let transaction = Self {
            source_hash: value.source_hash.ok_or(ConversionError::SourceHash)?,
            from: value.l1.from,
            to: if let Some(to) = value.l1.to {
                TxKind::Call(to)
            } else {
                TxKind::Create
            },
            mint: value.mint.unwrap_or_default(),
            value: value.l1.value,
            gas_limit: value.l1.gas.to(),
            is_system_tx: value.is_system_tx.unwrap_or(false),
            data: value.l1.input,
            hash: OnceLock::new(),
            rlp_encoding: OnceLock::new(),
        };

        Ok(transaction)
    }
}

/// Error that occurs when trying to convert the JSON-RPC `Transaction` type.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    /// L1 conversion error
    #[error(transparent)]
    L1(#[from] edr_chain_l1::rpc::transaction::RpcTransactionConversionError),
    /// Missing mint
    #[error("Missing mint")]
    Mint,
    /// Missing signature R-value.
    #[error("Missing signature R-value")]
    SignatureR,
    /// Missing signature S-value.
    #[error("Missing signature S-value")]
    SignatureS,
    /// Missing signature V-value.
    #[error("Missing signature V-value")]
    SignatureV,
    /// Missing source hash
    #[error("Missing source hash")]
    SourceHash,
}

impl TryFrom<Transaction> for transaction::OpSignedTransaction {
    type Error = ConversionError;

    fn try_from(value: Transaction) -> Result<Self, Self::Error> {
        let transaction_type = match value.l1.transaction_type.map_or(
            Ok(transaction::OpTransactionType::Legacy),
            transaction::OpTransactionType::try_from,
        ) {
            Ok(r#type) => r#type,
            Err(r#type) => {
                log::warn!(
                    "Unsupported transaction type: {type}. Reverting to post-EIP 155 legacy transaction"
                );

                // As the transaction type is not 0 or `None`, this will always result in a
                // post-EIP 155 legacy transaction.
                transaction::OpTransactionType::Legacy
            }
        };

        let transaction = match transaction_type {
            transaction::OpTransactionType::Deposit => Self::Deposit(value.try_into()?),
            transaction_type => {
                let r = value.r.ok_or(ConversionError::SignatureR)?;
                let s = value.s.ok_or(ConversionError::SignatureS)?;
                let v = value.v.ok_or(ConversionError::SignatureV)?;

                let transaction_with_signature =
                    L1RpcTransactionWithSignature::new(value.l1, r, s, v, value.y_parity);

                match transaction_type {
                    transaction::OpTransactionType::Legacy => {
                        if transaction_with_signature.is_legacy() {
                            Self::PreEip155Legacy(transaction_with_signature.into())
                        } else {
                            Self::PostEip155Legacy(transaction_with_signature.into())
                        }
                    }
                    transaction::OpTransactionType::Eip2930 => {
                        Self::Eip2930(transaction_with_signature.try_into()?)
                    }
                    transaction::OpTransactionType::Eip1559 => {
                        Self::Eip1559(transaction_with_signature.try_into()?)
                    }
                    transaction::OpTransactionType::Eip4844 => {
                        Self::Eip4844(transaction_with_signature.try_into()?)
                    }
                    transaction::OpTransactionType::Eip7702 => {
                        Self::Eip7702(transaction_with_signature.try_into()?)
                    }
                    transaction::OpTransactionType::Deposit => unreachable!("already handled"),
                }
            }
        };

        Ok(transaction)
    }
}

impl<BlockT: Block<OpSignedTransaction>>
    RpcTypeFrom<TransactionAndBlock<BlockT, OpSignedTransaction>> for Transaction
{
    type Hardfork = Hardfork;

    fn rpc_type_from(
        value: &TransactionAndBlock<BlockT, OpSignedTransaction>,
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

        let l1 = L1RpcTransaction::new(
            &value.transaction,
            header,
            transaction_index,
            value.is_pending,
            hardfork.into(),
        );

        let signature = value.transaction.maybe_signature();

        let is_system_tx =
            if l1.transaction_type == Some(transaction::OpTransactionType::Deposit.into()) {
                Some(value.transaction.is_system_transaction())
            } else {
                None
            };

        Self {
            l1,
            v: signature.map(Signature::v),
            // Following Hardhat in always returning `v` instead of `y_parity`.
            y_parity: None,
            r: signature.map(Signature::r),
            s: signature.map(Signature::s),
            source_hash: value.transaction.source_hash(),
            mint: value.transaction.mint(),
            is_system_tx,
        }
    }
}
