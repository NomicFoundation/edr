use edr_eth::{transaction::SignedTransaction as _, SpecId};
use edr_evm::{
    block::transaction::{BlockDataForTransaction, TransactionAndBlock},
    transaction::remote::EthRpcTransaction,
};
use edr_rpc_eth::RpcTypeFrom;
use serde::{Deserialize, Serialize};

use crate::GenericChainSpec;

// We need to use a newtype here as `RpcTypeFrom` cannot be implemented here,
// in an external crate, even though `TransactionAndBlock` is generic over
// a type that we introduced.
// This originally works as the impl for `L1ChainSpec` lives already in the
// defining crate of `edr_evm::TransactionAndBlock`, which probably shouldn't
// as far as defining spec externally is concerned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TransactionWithSignature(edr_rpc_eth::TransactionWithSignature);

impl EthRpcTransaction for TransactionWithSignature {
    fn block_hash(&self) -> Option<&edr_eth::B256> {
        self.0.block_hash()
    }
}

impl From<edr_rpc_eth::TransactionWithSignature> for TransactionWithSignature {
    fn from(value: edr_rpc_eth::TransactionWithSignature) -> Self {
        Self(value)
    }
}

impl RpcTypeFrom<TransactionAndBlock<GenericChainSpec>> for TransactionWithSignature {
    type Hardfork = SpecId;

    fn rpc_type_from(
        value: &TransactionAndBlock<GenericChainSpec>,
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

        let transaction = edr_rpc_eth::Transaction::new(
            &value.transaction,
            header,
            transaction_index,
            value.is_pending,
            hardfork,
        );
        let signature = value.transaction.0.signature();

        edr_rpc_eth::TransactionWithSignature::new(
            transaction,
            signature.r(),
            signature.s(),
            signature.v(),
            signature.y_parity(),
        )
        .into()
    }
}

pub use edr_rpc_eth::TransactionConversionError as ConversionError;

impl TryFrom<TransactionWithSignature> for crate::transaction::SignedWithFallbackToPostEip155 {
    type Error = ConversionError;

    fn try_from(value: TransactionWithSignature) -> Result<Self, Self::Error> {
        use edr_eth::transaction::{self, Signed};

        enum TxType {
            Known(transaction::Type),
            Unknown,
        }

        let TransactionWithSignature(value) = value;

        let tx_type = match value.transaction_type.map(transaction::Type::try_from) {
            None => TxType::Known(transaction::Type::Legacy),
            Some(Ok(r#type)) => TxType::Known(r#type),
            Some(Err(r#type)) => {
                log::warn!("Unsupported transaction type: {type}. Reverting to post-EIP 155 legacy transaction");

                TxType::Unknown
            }
        };

        let transaction = match tx_type {
            // We explicitly treat unknown transaction types as post-EIP 155 legacy transactions
            TxType::Unknown => Signed::PostEip155Legacy(value.into()),

            TxType::Known(transaction::Type::Legacy) => {
                if value.is_legacy() {
                    Signed::PreEip155Legacy(value.into())
                } else {
                    Signed::PostEip155Legacy(value.into())
                }
            }
            TxType::Known(transaction::Type::Eip2930) => Signed::Eip2930(value.try_into()?),
            TxType::Known(transaction::Type::Eip1559) => Signed::Eip1559(value.try_into()?),
            TxType::Known(transaction::Type::Eip4844) => Signed::Eip4844(value.try_into()?),
        };

        Ok(Self(transaction))
    }
}
