mod detailed;
/// Types for transactions from a remote provider.
pub mod remote;

use std::fmt::Debug;

// Re-export the transaction types from `edr_eth`.
pub use edr_eth::transaction::*;
use edr_eth::{eips::eip7702, transaction, SpecId, U256};
use revm::{
    db::DatabaseComponentError,
    interpreter::gas::calculate_initial_tx_gas,
    primitives::{EVMError, InvalidHeader, InvalidTransaction},
};

pub use self::detailed::*;

/// Invalid transaction error
#[derive(Debug, thiserror::Error)]
pub enum TransactionError<BE, SE> {
    /// Blockchain errors
    #[error(transparent)]
    Blockchain(#[from] BE),
    #[error("{0}")]
    /// Custom errors
    Custom(String),
    /// EIP-1559 is not supported
    #[error("Cannot run transaction: EIP 1559 is not activated.")]
    Eip1559Unsupported,
    /// Corrupt transaction data
    #[error("Invalid transaction: {0:?}")]
    InvalidTransaction(InvalidTransaction),
    /// The transaction is expected to have a prevrandao, as the executor's
    /// config is on a post-merge hardfork.
    #[error("Post-merge transaction is missing prevrandao")]
    MissingPrevrandao,
    /// Precompile errors
    #[error("{0}")]
    Precompile(String),
    /// State errors
    #[error(transparent)]
    State(SE),
}

impl<BE, SE> From<EVMError<DatabaseComponentError<SE, BE>>> for TransactionError<BE, SE>
where
    BE: Debug + Send,
    SE: Debug + Send,
{
    fn from(error: EVMError<DatabaseComponentError<SE, BE>>) -> Self {
        match error {
            EVMError::Transaction(e) => Self::InvalidTransaction(e),
            EVMError::Header(
                InvalidHeader::ExcessBlobGasNotSet | InvalidHeader::PrevrandaoNotSet,
            ) => unreachable!("error: {error:?}"),
            EVMError::Database(DatabaseComponentError::State(e)) => Self::State(e),
            EVMError::Database(DatabaseComponentError::BlockHash(e)) => Self::Blockchain(e),
            EVMError::Custom(error) => Self::Custom(error),
            EVMError::Precompile(error) => Self::Precompile(error),
        }
    }
}

/// An error that occurred while during [`validate`].
#[derive(Debug, thiserror::Error)]
pub enum CreationError {
    /// Creating contract without any data.
    #[error("Contract creation without any data provided")]
    ContractMissingData,
    /// Transaction gas limit is insufficient to afford initial gas cost.
    #[error("Transaction requires at least {initial_gas_cost} gas but got {gas_limit}")]
    InsufficientGas {
        /// The initial gas cost of a transaction
        initial_gas_cost: U256,
        /// The gas limit of the transaction
        gas_limit: u64,
    },
}

/// Validates the transaction.
pub fn validate(
    transaction: transaction::Signed,
    spec_id: SpecId,
) -> Result<transaction::Signed, CreationError> {
    if transaction.kind() == TxKind::Create && transaction.data().is_empty() {
        return Err(CreationError::ContractMissingData);
    }

    let initial_cost = initial_cost(&transaction, spec_id);
    if transaction.gas_limit() < initial_cost {
        return Err(CreationError::InsufficientGas {
            initial_gas_cost: U256::from(initial_cost),
            gas_limit: transaction.gas_limit(),
        });
    }

    Ok(transaction)
}

/// Calculates the initial cost of a transaction.
pub fn initial_cost(transaction: &transaction::Signed, spec_id: SpecId) -> u64 {
    let authorization_list_num = transaction
        .authorization_list()
        .map(<[eip7702::SignedAuthorization]>::len)
        // usize is guaranteed to fit into u64
        .unwrap_or_default() as u64;

    calculate_initial_tx_gas(
        spec_id,
        transaction.data().as_ref(),
        transaction.kind() == TxKind::Create,
        transaction.access_list(),
        authorization_list_num,
    )
    .initial_gas
}

#[cfg(test)]
mod tests {
    use edr_eth::{transaction, Address, Bytes};

    use super::*;

    #[test]
    fn gas_limit_less_than_base_fee() -> anyhow::Result<()> {
        const TOO_LOW_GAS_LIMIT: u64 = 100;

        let caller = Address::random();

        let request = transaction::request::Eip155 {
            nonce: 0,
            gas_price: U256::ZERO,
            gas_limit: TOO_LOW_GAS_LIMIT,
            kind: TxKind::Call(caller),
            value: U256::ZERO,
            input: Bytes::new(),
            chain_id: 123,
        };

        let transaction = request.fake_sign(caller);
        let transaction = transaction::Signed::from(transaction);
        let result = validate(transaction, SpecId::BERLIN);

        let expected_gas_cost = U256::from(21_000);
        assert!(matches!(
            result,
            Err(CreationError::InsufficientGas {
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

        let request = transaction::request::Eip155 {
            nonce: 0,
            gas_price: U256::ZERO,
            gas_limit: 30_000,
            kind: TxKind::Create,
            value: U256::ZERO,
            input: Bytes::new(),
            chain_id: 123,
        };

        let transaction = request.fake_sign(caller);
        let transaction = transaction::Signed::from(transaction);
        let result = validate(transaction, SpecId::BERLIN);

        assert!(matches!(result, Err(CreationError::ContractMissingData)));

        assert_eq!(
            result.unwrap_err().to_string(),
            "Contract creation without any data provided"
        );

        Ok(())
    }
}
