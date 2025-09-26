mod detailed;
/// Types for transactions from a remote provider.
pub mod remote;

use std::fmt::Debug;

use edr_evm_spec::{
    ChainSpec, EvmHeaderValidationError, EvmSpecId, EvmTransactionValidationError,
    TransactionValidation,
};
use edr_primitives::U256;
use edr_state_api::database::DatabaseComponentError;
use edr_transaction::TxKind;
use revm_handler::validation::validate_initial_tx_gas;
pub use revm_interpreter::gas::calculate_initial_tx_gas_for_tx;

pub use self::detailed::*;

/// Helper type for a chain-specific [`TransactionError`].
pub type TransactionErrorForChainSpec<BlockchainErrorT, ChainSpecT, StateErrorT> = TransactionError<
    BlockchainErrorT,
    StateErrorT,
    <<ChainSpecT as ChainSpec>::SignedTransaction as TransactionValidation>::ValidationError,
>;

/// Invalid transaction error
#[derive(Debug, thiserror::Error)]
pub enum TransactionError<BlockchainErrorT, StateErrorT, TransactionValidationErrorT> {
    /// Blockchain errors
    #[error(transparent)]
    Blockchain(BlockchainErrorT),
    /// Custom errors
    #[error("{0}")]
    Custom(String),
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
    /// State errors
    #[error(transparent)]
    State(StateErrorT),
}

impl<BlockchainErrorT, StateErrorT, TransactionValidationErrorT>
    From<DatabaseComponentError<BlockchainErrorT, StateErrorT>>
    for TransactionError<BlockchainErrorT, StateErrorT, TransactionValidationErrorT>
{
    fn from(value: DatabaseComponentError<BlockchainErrorT, StateErrorT>) -> Self {
        match value {
            DatabaseComponentError::Blockchain(e) => Self::Blockchain(e),
            DatabaseComponentError::State(e) => Self::State(e),
        }
    }
}

impl<BlockchainErrorT, StateErrorT, TransactionValidationErrorT> From<EvmHeaderValidationError>
    for TransactionError<BlockchainErrorT, StateErrorT, TransactionValidationErrorT>
{
    fn from(value: EvmHeaderValidationError) -> Self {
        Self::InvalidHeader(value)
    }
}

impl<BlockchainErrorT, StateErrorT> From<EvmTransactionValidationError>
    for TransactionError<BlockchainErrorT, StateErrorT, EvmTransactionValidationError>
{
    fn from(value: EvmTransactionValidationError) -> Self {
        match value {
            EvmTransactionValidationError::LackOfFundForMaxFee { fee, balance } => {
                Self::LackOfFundForMaxFee { fee, balance }
            }
            remainder => Self::InvalidTransaction(remainder),
        }
    }
}

/// An error that occurred while during [`validate`].
#[derive(Debug, thiserror::Error)]
pub enum CreationError {
    /// Creating contract without any data.
    #[error("Contract creation without any data provided")]
    ContractMissingData,
    /// Transaction gas limit is insufficient for gas floor.
    #[error("Transaction requires gas floor of {gas_floor} but got limit of {gas_limit}")]
    GasFloorTooHigh {
        /// The gas floor of the transaction
        gas_floor: u64,
        /// The gas limit of the transaction
        gas_limit: u64,
    },
    /// Transaction gas limit is insufficient to afford initial gas cost.
    #[error("Transaction requires at least {initial_gas_cost} gas but got {gas_limit}")]
    InsufficientGas {
        /// The initial gas cost of a transaction
        initial_gas_cost: u64,
        /// The gas limit of the transaction
        gas_limit: u64,
    },
}

/// Validates the transaction.
pub fn validate<TransactionT: revm_context_interface::Transaction>(
    transaction: TransactionT,
    spec_id: EvmSpecId,
) -> Result<TransactionT, CreationError> {
    if transaction.kind() == TxKind::Create && transaction.input().is_empty() {
        return Err(CreationError::ContractMissingData);
    }

    match validate_initial_tx_gas(&transaction, spec_id) {
        Ok(_) => Ok(transaction),
        Err(EvmTransactionValidationError::CallGasCostMoreThanGasLimit {
            initial_gas,
            gas_limit,
        }) => Err(CreationError::InsufficientGas {
            initial_gas_cost: initial_gas,
            gas_limit,
        }),
        Err(EvmTransactionValidationError::GasFloorMoreThanGasLimit {
            gas_floor,
            gas_limit,
        }) => Err(CreationError::GasFloorTooHigh {
            gas_floor,
            gas_limit,
        }),
        Err(e) => unreachable!("Unexpected error: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use edr_primitives::{Address, Bytes};

    use super::*;

    #[test]
    fn gas_limit_less_than_base_fee() -> anyhow::Result<()> {
        const TOO_LOW_GAS_LIMIT: u64 = 100;

        let caller = Address::random();

        let request = edr_chain_l1::request::Eip155 {
            nonce: 0,
            gas_price: 0,
            gas_limit: TOO_LOW_GAS_LIMIT,
            kind: TxKind::Call(caller),
            value: U256::ZERO,
            input: Bytes::new(),
            chain_id: 123,
        };

        let transaction = request.fake_sign(caller);
        let transaction = edr_chain_l1::L1SignedTransaction::from(transaction);
        let result = validate(transaction, EvmSpecId::BERLIN);

        let expected_gas_cost = 21_000;
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

        let request = edr_chain_l1::request::Eip155 {
            nonce: 0,
            gas_price: 0,
            gas_limit: 30_000,
            kind: TxKind::Create,
            value: U256::ZERO,
            input: Bytes::new(),
            chain_id: 123,
        };

        let transaction = request.fake_sign(caller);
        let transaction = edr_chain_l1::L1SignedTransaction::from(transaction);
        let result = validate(transaction, EvmSpecId::BERLIN);

        assert!(matches!(result, Err(CreationError::ContractMissingData)));

        assert_eq!(
            result.unwrap_err().to_string(),
            "Contract creation without any data provided"
        );

        Ok(())
    }
}
