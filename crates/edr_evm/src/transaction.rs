mod detailed;
/// Types for transactions from a remote provider.
pub mod remote;

use std::fmt::Debug;

use derive_where::derive_where;
// Re-export the transaction types from `edr_eth`.
pub use edr_eth::transaction::*;
use edr_eth::{l1, spec::ChainSpec, U256};
use revm::precompile::PrecompileErrors;

pub use self::detailed::*;
use crate::state::DatabaseComponentError;

/// Invalid transaction error
#[derive(thiserror::Error)]
#[derive_where(Debug; <ChainSpecT::SignedTransaction as TransactionValidation>::ValidationError, BlockchainErrorT, StateErrorT)]
pub enum TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>
where
    ChainSpecT: ChainSpec,
{
    /// Blockchain errors
    #[error(transparent)]
    Blockchain(BlockchainErrorT),
    /// Custom errors
    #[error("{0}")]
    Custom(String),
    /// Invalid block header
    #[error(transparent)]
    InvalidHeader(l1::InvalidHeader),
    /// Corrupt transaction data
    #[error(transparent)]
    InvalidTransaction(<ChainSpecT::SignedTransaction as TransactionValidation>::ValidationError),
    /// Transaction account does not have enough amount of ether to cover
    /// transferred value and gas_limit*gas_price.
    #[error("Sender doesn't have enough funds to send tx. The max upfront cost is: {fee} and the sender's balance is: {balance}.")]
    LackOfFundForMaxFee {
        /// The max upfront cost of the transaction
        fee: Box<U256>,
        /// The sender's balance
        balance: Box<U256>,
    },
    /// Precompile errors
    #[error("{0}")]
    Precompile(PrecompileErrors),
    /// State errors
    #[error(transparent)]
    State(StateErrorT),
}

impl<BlockchainErrorT, ChainSpecT, StateErrorT>
    From<DatabaseComponentError<BlockchainErrorT, StateErrorT>>
    for TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>
where
    ChainSpecT: ChainSpec,
{
    fn from(value: DatabaseComponentError<BlockchainErrorT, StateErrorT>) -> Self {
        match value {
            DatabaseComponentError::Blockchain(e) => Self::Blockchain(e),
            DatabaseComponentError::State(e) => Self::State(e),
        }
    }
}

impl<BlockchainErrorT, ChainSpecT, StateErrorT> From<l1::InvalidHeader>
    for TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>
where
    ChainSpecT: ChainSpec,
{
    fn from(value: l1::InvalidHeader) -> Self {
        Self::InvalidHeader(value)
    }
}

impl<BlockchainErrorT, ChainSpecT, StateErrorT> From<l1::InvalidTransaction>
    for TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>
where
    ChainSpecT: ChainSpec<
        SignedTransaction: TransactionValidation<ValidationError: From<l1::InvalidTransaction>>,
    >,
{
    fn from(value: l1::InvalidTransaction) -> Self {
        match value {
            l1::InvalidTransaction::LackOfFundForMaxFee { fee, balance } => {
                Self::LackOfFundForMaxFee { fee, balance }
            }
            remainder => Self::InvalidTransaction(remainder.into()),
        }
    }
}

impl<BlockchainErrorT, ChainSpecT, StateErrorT> From<PrecompileErrors>
    for TransactionError<BlockchainErrorT, ChainSpecT, StateErrorT>
where
    ChainSpecT: ChainSpec,
{
    fn from(value: PrecompileErrors) -> Self {
        Self::Precompile(value)
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
pub fn validate<TransactionT: revm_context_interface::Transaction>(
    transaction: TransactionT,
    spec_id: l1::SpecId,
) -> Result<TransactionT, CreationError> {
    if transaction.kind() == TxKind::Create && transaction.input().is_empty() {
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

// TODO: Avoid using revm Transaction type
/// Calculates the initial cost of a transaction.
pub fn initial_cost(
    transaction: &impl revm_context_interface::Transaction,
    spec_id: l1::SpecId,
) -> u64 {
    let (accounts, storages) = transaction.access_list_nums().unwrap_or_default();

    revm_interpreter::gas::calculate_initial_tx_gas(
        spec_id,
        transaction.input(),
        transaction.kind().is_create(),
        accounts as u64,
        storages as u64,
        transaction.authorization_list_len() as u64,
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
            gas_price: 0,
            gas_limit: TOO_LOW_GAS_LIMIT,
            kind: TxKind::Call(caller),
            value: U256::ZERO,
            input: Bytes::new(),
            chain_id: 123,
        };

        let transaction = request.fake_sign(caller);
        let transaction = transaction::Signed::from(transaction);
        let result = validate(transaction, l1::SpecId::BERLIN);

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
            gas_price: 0,
            gas_limit: 30_000,
            kind: TxKind::Create,
            value: U256::ZERO,
            input: Bytes::new(),
            chain_id: 123,
        };

        let transaction = request.fake_sign(caller);
        let transaction = transaction::Signed::from(transaction);
        let result = validate(transaction, l1::SpecId::BERLIN);

        assert!(matches!(result, Err(CreationError::ContractMissingData)));

        assert_eq!(
            result.unwrap_err().to_string(),
            "Contract creation without any data provided"
        );

        Ok(())
    }
}
