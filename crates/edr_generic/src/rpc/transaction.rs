use edr_block_api::Block;
use edr_chain_l1::rpc::transaction::{L1RpcTransaction, L1RpcTransactionWithSignature};
use edr_primitives::B256;
use edr_rpc_spec::{RpcTransaction, RpcTypeFrom};
use edr_transaction::{BlockDataForTransaction, SignedTransaction as _, TransactionAndBlock};
use serde::{Deserialize, Serialize};

use crate::transaction::{self, SignedTransactionWithFallbackToPostEip155};

// We need to use a newtype here as `RpcTypeFrom` cannot be implemented here,
// in an external crate, even though `TransactionAndBlock` is generic over
// a type that we introduced.
// This originally works as the impl for `L1ChainSpec` lives already in the
// defining crate of `edr_evm::TransactionAndBlock`, which probably shouldn't
// as far as defining spec externally is concerned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GenericRpcTransactionWithSignature(L1RpcTransactionWithSignature);

impl RpcTransaction for GenericRpcTransactionWithSignature {
    fn block_hash(&self) -> Option<&B256> {
        self.0.block_hash()
    }
}

impl From<L1RpcTransactionWithSignature> for GenericRpcTransactionWithSignature {
    fn from(value: L1RpcTransactionWithSignature) -> Self {
        Self(value)
    }
}

impl<BlockT: Block<SignedTransactionWithFallbackToPostEip155>>
    RpcTypeFrom<TransactionAndBlock<BlockT, SignedTransactionWithFallbackToPostEip155>>
    for GenericRpcTransactionWithSignature
{
    type Hardfork = edr_chain_l1::Hardfork;

    fn rpc_type_from(
        value: &TransactionAndBlock<BlockT, SignedTransactionWithFallbackToPostEip155>,
        hardfork: Self::Hardfork,
    ) -> Self {
        let (header, transaction_index) = value
            .block_data
            .as_ref()
            .map(
                |BlockDataForTransaction {
                     block,
                     transaction_index,
                 }| (block.header(), *transaction_index),
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
        .into()
    }
}

/// Error that occurs when trying to convert the JSON-RPC `Transaction` type.
pub type GenericRpcTransactionConversionError =
    edr_chain_l1::rpc::transaction::RpcTransactionConversionError;

impl TryFrom<GenericRpcTransactionWithSignature>
    for transaction::SignedTransactionWithFallbackToPostEip155
{
    type Error = GenericRpcTransactionConversionError;

    fn try_from(value: GenericRpcTransactionWithSignature) -> Result<Self, Self::Error> {
        use edr_chain_l1::L1SignedTransaction;

        let GenericRpcTransactionWithSignature(value) = value;

        let tx_type = match value
            .transaction_type
            .map(edr_chain_l1::L1TransactionType::try_from)
        {
            None => transaction::Type::Legacy,
            Some(Ok(r#type)) => r#type.into(),
            Some(Err(r#type)) => {
                log::warn!(
                    "Unsupported transaction type: {type}. Reverting to post-EIP 155 legacy transaction"
                );

                transaction::Type::Unrecognized(r#type)
            }
        };

        let transaction = match tx_type {
            // We explicitly treat unrecognized transaction types as post-EIP 155 legacy
            // transactions
            transaction::Type::Unrecognized(_) => {
                L1SignedTransaction::PostEip155Legacy(value.into())
            }

            transaction::Type::Legacy => {
                if value.is_legacy() {
                    L1SignedTransaction::PreEip155Legacy(value.into())
                } else {
                    L1SignedTransaction::PostEip155Legacy(value.into())
                }
            }
            transaction::Type::Eip2930 => L1SignedTransaction::Eip2930(value.try_into()?),
            transaction::Type::Eip1559 => L1SignedTransaction::Eip1559(value.try_into()?),
            transaction::Type::Eip4844 => L1SignedTransaction::Eip4844(value.try_into()?),
            transaction::Type::Eip7702 => L1SignedTransaction::Eip7702(value.try_into()?),
        };

        Ok(Self::with_type(transaction, tx_type))
    }
}
