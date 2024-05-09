use std::sync::OnceLock;

use alloy_rlp::BufMut;
use edr_eth::{
    remote,
    signature::Signature,
    transaction::{
        Eip1559SignedTransaction, Eip155SignedTransaction, Eip2930SignedTransaction,
        Eip4844SignedTransaction, LegacySignedTransaction, SignedTransaction, Transaction,
        TransactionType, TxKind,
    },
    Address, B256, U256,
};
use revm::{
    interpreter::gas::validate_initial_tx_gas,
    primitives::{SpecId, TxEnv},
};

use super::TransactionCreationError;

/// A transaction that can be executed by the EVM. It allows manual
/// specification of the caller, e.g. to override the caller of a transaction
/// that can be recovered from a signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutableTransaction {
    transaction: SignedTransaction,
    caller: Address,
}

impl ExecutableTransaction {
    /// Create an [`ExecutableTransaction`] by attempting to validate and
    /// recover the caller address of the provided transaction.
    pub fn new(
        spec_id: SpecId,
        transaction: SignedTransaction,
    ) -> Result<Self, TransactionCreationError> {
        let caller = transaction
            .recover()
            .map_err(TransactionCreationError::Signature)?;

        Self::with_caller(spec_id, transaction, caller)
    }

    /// Creates an [`ExecutableTransaction`] with the provided transaction and
    /// caller address.
    pub fn with_caller(
        spec_id: SpecId,
        transaction: SignedTransaction,
        caller: Address,
    ) -> Result<Self, TransactionCreationError> {
        if transaction.kind() == TxKind::Create && transaction.data().is_empty() {
            return Err(TransactionCreationError::ContractMissingData);
        }

        let initial_cost = initial_cost(spec_id, &transaction);
        if transaction.gas_limit() < initial_cost {
            return Err(TransactionCreationError::InsufficientGas {
                initial_gas_cost: U256::from(initial_cost),
                gas_limit: transaction.gas_limit(),
            });
        }

        Ok(Self {
            transaction,
            caller,
        })
    }

    /// Returns the [`ExecutableTransaction`]'s caller.
    pub fn caller(&self) -> &Address {
        &self.caller
    }

    /// The minimum gas required to include the transaction in a block.
    pub fn initial_cost(&self, spec_id: SpecId) -> u64 {
        initial_cost(spec_id, &self.transaction)
    }

    /// Returns the inner [`SignedTransaction`]
    pub fn as_inner(&self) -> &SignedTransaction {
        &self.transaction
    }

    /// Returns the inner transaction and caller
    pub fn into_inner(self) -> (SignedTransaction, Address) {
        (self.transaction, self.caller)
    }
}

impl alloy_rlp::Encodable for ExecutableTransaction {
    fn encode(&self, out: &mut dyn BufMut) {
        self.transaction.encode(out);
    }

    fn length(&self) -> usize {
        self.transaction.length()
    }
}

impl From<ExecutableTransaction> for TxEnv {
    fn from(value: ExecutableTransaction) -> Self {
        value.transaction.into_tx_env(value.caller)
    }
}

impl Transaction for ExecutableTransaction {
    fn effective_gas_price(&self, block_base_fee: U256) -> U256 {
        self.transaction.effective_gas_price(block_base_fee)
    }

    fn gas_limit(&self) -> u64 {
        self.transaction.gas_limit()
    }

    fn gas_price(&self) -> U256 {
        self.transaction.gas_price()
    }

    fn max_fee_per_gas(&self) -> Option<U256> {
        self.transaction.max_fee_per_gas()
    }

    fn max_fee_per_blob_gas(&self) -> Option<U256> {
        self.transaction.max_fee_per_blob_gas()
    }

    fn max_priority_fee_per_gas(&self) -> Option<U256> {
        self.transaction.max_priority_fee_per_gas()
    }

    fn nonce(&self) -> u64 {
        self.transaction.nonce()
    }

    fn to(&self) -> Option<Address> {
        self.transaction.to()
    }

    fn total_blob_gas(&self) -> Option<u64> {
        self.transaction.total_blob_gas()
    }

    fn transaction_hash(&self) -> &B256 {
        self.transaction.transaction_hash()
    }

    fn transaction_type(&self) -> TransactionType {
        self.transaction.transaction_type()
    }

    fn value(&self) -> U256 {
        self.transaction.value()
    }
}

/// Error that occurs when trying to convert the JSON-RPC `Transaction` type.
#[derive(Debug, thiserror::Error)]
pub enum TransactionConversionError {
    /// Missing access list
    #[error("Missing access list")]
    MissingAccessList,
    /// EIP-4844 transaction is missing blob (versioned) hashes
    #[error("Missing blob hashes")]
    MissingBlobHashes,
    /// Missing chain ID
    #[error("Missing chain ID")]
    MissingChainId,
    /// Missing max fee per gas
    #[error("Missing max fee per gas")]
    MissingMaxFeePerGas,
    /// Missing max priority fee per gas
    #[error("Missing max priority fee per gas")]
    MissingMaxPriorityFeePerGas,
    /// EIP-4844 transaction is missing the max fee per blob gas
    #[error("Missing max fee per blob gas")]
    MissingMaxFeePerBlobGas,
    /// EIP-4844 transaction is missing the receiver (to) address
    #[error("Missing receiver (to) address")]
    MissingReceiverAddress,
}

impl TryFrom<remote::eth::Transaction> for ExecutableTransaction {
    type Error = TransactionConversionError;

    fn try_from(value: remote::eth::Transaction) -> Result<Self, Self::Error> {
        let kind = if let Some(to) = &value.to {
            TxKind::Call(*to)
        } else {
            TxKind::Create
        };

        let caller = value.from;

        let transaction = match value.transaction_type {
            Some(0) | None => {
                if value.is_legacy() {
                    SignedTransaction::PreEip155Legacy(LegacySignedTransaction {
                        nonce: value.nonce,
                        gas_price: value.gas_price,
                        gas_limit: value.gas.to(),
                        kind,
                        value: value.value,
                        input: value.input,
                        signature: Signature {
                            r: value.r,
                            s: value.s,
                            v: value.v,
                        },
                        hash: OnceLock::from(value.hash),
                        is_fake: false,
                    })
                } else {
                    SignedTransaction::PostEip155Legacy(Eip155SignedTransaction {
                        nonce: value.nonce,
                        gas_price: value.gas_price,
                        gas_limit: value.gas.to(),
                        kind,
                        value: value.value,
                        input: value.input,
                        signature: Signature {
                            r: value.r,
                            s: value.s,
                            v: value.v,
                        },
                        hash: OnceLock::from(value.hash),
                        is_fake: false,
                    })
                }
            }
            Some(1) => SignedTransaction::Eip2930(Eip2930SignedTransaction {
                odd_y_parity: value.odd_y_parity(),
                chain_id: value
                    .chain_id
                    .ok_or(TransactionConversionError::MissingChainId)?,
                nonce: value.nonce,
                gas_price: value.gas_price,
                gas_limit: value.gas.to(),
                kind,
                value: value.value,
                input: value.input,
                access_list: value
                    .access_list
                    .ok_or(TransactionConversionError::MissingAccessList)?
                    .into(),
                r: value.r,
                s: value.s,
                hash: OnceLock::from(value.hash),
                is_fake: false,
            }),
            Some(2) => SignedTransaction::Eip1559(Eip1559SignedTransaction {
                odd_y_parity: value.odd_y_parity(),
                chain_id: value
                    .chain_id
                    .ok_or(TransactionConversionError::MissingChainId)?,
                nonce: value.nonce,
                max_priority_fee_per_gas: value
                    .max_priority_fee_per_gas
                    .ok_or(TransactionConversionError::MissingMaxPriorityFeePerGas)?,
                max_fee_per_gas: value
                    .max_fee_per_gas
                    .ok_or(TransactionConversionError::MissingMaxFeePerGas)?,
                gas_limit: value.gas.to(),
                kind,
                value: value.value,
                input: value.input,
                access_list: value
                    .access_list
                    .ok_or(TransactionConversionError::MissingAccessList)?
                    .into(),
                r: value.r,
                s: value.s,
                hash: OnceLock::from(value.hash),
                is_fake: false,
            }),
            Some(3) => SignedTransaction::Eip4844(Eip4844SignedTransaction {
                odd_y_parity: value.odd_y_parity(),
                chain_id: value
                    .chain_id
                    .ok_or(TransactionConversionError::MissingChainId)?,
                nonce: value.nonce,
                max_priority_fee_per_gas: value
                    .max_priority_fee_per_gas
                    .ok_or(TransactionConversionError::MissingMaxPriorityFeePerGas)?,
                max_fee_per_gas: value
                    .max_fee_per_gas
                    .ok_or(TransactionConversionError::MissingMaxFeePerGas)?,
                max_fee_per_blob_gas: value
                    .max_fee_per_blob_gas
                    .ok_or(TransactionConversionError::MissingMaxFeePerBlobGas)?,
                gas_limit: value.gas.to(),
                to: value
                    .to
                    .ok_or(TransactionConversionError::MissingReceiverAddress)?,
                value: value.value,
                input: value.input,
                access_list: value
                    .access_list
                    .ok_or(TransactionConversionError::MissingAccessList)?
                    .into(),
                blob_hashes: value
                    .blob_versioned_hashes
                    .ok_or(TransactionConversionError::MissingBlobHashes)?,
                r: value.r,
                s: value.s,
                hash: OnceLock::from(value.hash),
                is_fake: false,
            }),
            Some(r#type) => {
                log::warn!("Unsupported transaction type: {type}. Reverting to post-EIP 155 legacy transaction", );

                SignedTransaction::PostEip155Legacy(Eip155SignedTransaction {
                    nonce: value.nonce,
                    gas_price: value.gas_price,
                    gas_limit: value.gas.to(),
                    kind,
                    value: value.value,
                    input: value.input,
                    signature: Signature {
                        r: value.r,
                        s: value.s,
                        v: value.v,
                    },
                    hash: OnceLock::from(value.hash),
                    is_fake: false,
                })
            }
        };

        Ok(ExecutableTransaction {
            transaction,
            caller,
        })
    }
}

fn initial_cost(spec_id: SpecId, transaction: &SignedTransaction) -> u64 {
    let access_list = transaction
        .access_list()
        .cloned()
        .map(Vec::<(Address, Vec<U256>)>::from);

    validate_initial_tx_gas(
        spec_id,
        transaction.data().as_ref(),
        transaction.kind() == TxKind::Create,
        access_list
            .as_ref()
            .map_or(&[], |access_list| access_list.as_slice()),
        // TODO: https://github.com/NomicFoundation/edr/issues/427
        &[],
    )
}

#[cfg(test)]
mod tests {
    use edr_eth::{transaction::Eip155TransactionRequest, Bytes};

    use super::*;

    #[test]
    fn gas_limit_less_than_base_fee() -> anyhow::Result<()> {
        const TOO_LOW_GAS_LIMIT: u64 = 100;

        let caller = Address::random();

        let request = Eip155TransactionRequest {
            nonce: 0,
            gas_price: U256::ZERO,
            gas_limit: TOO_LOW_GAS_LIMIT,
            kind: TxKind::Call(caller),
            value: U256::ZERO,
            input: Bytes::new(),
            chain_id: 123,
        };

        let transaction = request.fake_sign(&caller);
        let result = ExecutableTransaction::with_caller(SpecId::BERLIN, transaction.into(), caller);

        let expected_gas_cost = U256::from(21_000);
        assert!(matches!(
            result,
            Err(TransactionCreationError::InsufficientGas {
                initial_gas_cost,
                gas_limit: TOO_LOW_GAS_LIMIT,
            }) if initial_gas_cost == expected_gas_cost
        ));

        assert_eq!(
            result.unwrap_err().to_string(),
            format!("Transaction requires at least 21000 gas but got {TOO_LOW_GAS_LIMIT}")
        );

        Ok(())
    }

    #[test]
    fn create_missing_data() -> anyhow::Result<()> {
        let caller = Address::random();

        let request = Eip155TransactionRequest {
            nonce: 0,
            gas_price: U256::ZERO,
            gas_limit: 30_000,
            kind: TxKind::Create,
            value: U256::ZERO,
            input: Bytes::new(),
            chain_id: 123,
        };

        let transaction = request.fake_sign(&caller);
        let result = ExecutableTransaction::with_caller(SpecId::BERLIN, transaction.into(), caller);

        assert!(matches!(
            result,
            Err(TransactionCreationError::ContractMissingData)
        ));

        assert_eq!(
            result.unwrap_err().to_string(),
            "Contract creation without any data provided"
        );

        Ok(())
    }
}
