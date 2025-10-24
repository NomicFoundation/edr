use edr_chain_spec::{
    ChainSpec, EvmHeaderValidationError, EvmTransactionValidationError, TransactionValidation,
};
use edr_database_components::{DatabaseComponents, WrapDatabaseRef};
use edr_primitives::U256;
use revm_context::DBErrorMarker;

use crate::result::EVMError;

/// Invalid transaction error
#[derive(Debug, thiserror::Error)]
pub enum TransactionError<DatabaseErrorT, TransactionValidationErrorT> {
    /// Custom errors
    #[error("{0}")]
    Custom(String),
    /// Database error
    #[error(transparent)]
    Database(DatabaseErrorT),
    /// Invalid block header
    #[error(transparent)]
    InvalidHeader(EvmHeaderValidationError),
    /// Corrupt transaction data
    #[error(transparent)]
    InvalidTransaction(TransactionValidationErrorT),
    /// Transaction account does not have enough amount of ether to cover
    /// transferred value and `gas_limit * gas_price`.
    #[error(
        "Sender doesn't have enough funds to send tx. The max upfront cost is: {fee} and the sender's balance is: {balance}."
    )]
    LackOfFundForMaxFee {
        /// The max upfront cost of the transaction
        fee: Box<U256>,
        /// The sender's balance
        balance: Box<U256>,
    },
}

impl<ErrorT: DBErrorMarker + std::error::Error>
    From<EVMError<ErrorT, EvmTransactionValidationError>>
    for TransactionError<ErrorT, EvmTransactionValidationError>
{
    fn from(value: EVMError<ErrorT, EvmTransactionValidationError>) -> Self {
        match value {
            EVMError::Custom(error) => TransactionError::Custom(error),
            EVMError::Database(error) => TransactionError::Database(error),
            EVMError::Header(error) => TransactionError::InvalidHeader(error),
            EVMError::Transaction(EvmTransactionValidationError::LackOfFundForMaxFee {
                fee,
                balance,
            }) => TransactionError::LackOfFundForMaxFee { fee, balance },
            EVMError::Transaction(error) => TransactionError::InvalidTransaction(error),
        }
    }
}

/// Helper type for a chain-specific [`TransactionError`].
pub type TransactionErrorForChainSpec<BlockchainErrorT, ChainSpecT, StateErrorT> = TransactionError<
    WrapDatabaseRef<DatabaseComponents<BlockchainErrorT, StateErrorT>>,
    <<ChainSpecT as ChainSpec>::SignedTransaction as TransactionValidation>::ValidationError,
>;
