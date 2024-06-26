mod block;

use std::ops::Deref;

use edr_eth::{signature, B256, U128};
use edr_evm::transaction::{remote::EthRpcTransaction, TxKind};
use edr_rpc_eth::TransactionConversionError;

use crate::transaction;

/// Optimism RPC transaction.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    #[serde(flatten)]
    l1: edr_rpc_eth::Transaction,
    /// Hash that uniquely identifies the source of the deposit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<B256>,
    /// The ETH value to mint on L2
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mint: Option<U128>,
    /// Field indicating whether the transaction is a system transaction, and
    /// therefore exempt from the L2 gas limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_system_tx: Option<bool>,
}

impl Transaction {
    /// Returns whether the transaction is a legacy transaction.
    pub fn is_legacy(&self) -> bool {
        matches!(self.transaction_type, None | Some(0)) && matches!(self.v, 27 | 28)
    }
}


impl Deref for Transaction {
    type Target = edr_rpc_eth::Transaction;

    fn deref(&self) -> &Self::Target {
        &self.l1
    }
}

impl EthRpcTransaction for Transaction {
    fn block_hash(&self) -> Option<&B256> {
        self.l1.block_hash()
    }
}

impl TryFrom<Transaction> for transaction::Signed {
    type Error = TransactionConversionError;

    fn try_from(value: Transaction) -> Result<Self, Self::Error> {

        let transaction_type = match value.transaction_type.map_or(Ok(transaction::Type::Legacy), transaction::Type::try_from) {
            Ok(r#type) => r#type,
            Err(r#type) => {
                log::warn!("Unsupported transaction type: {type}. Reverting to post-EIP 155 legacy transaction");
    
                transaction::Type::Legacy
            }
        };
        
        let kind = if let Some(to) = &value.to {
            TxKind::Call(*to)
        } else {
            TxKind::Create
        };

        let transaction = match transaction_type {
            transaction::Type::Legacy =>  {
                if value.is_legacy() {
                    transaction::Signed::PreEip155Legacy(value.into())
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
            },
            transaction::Type::Eip2930 => todo!(),
            transaction::Type::Eip1559 => todo!(),
            transaction::Type::Eip4844 => todo!(),
            transaction::Type::Deposited => todo!(),
        }
    }
}
