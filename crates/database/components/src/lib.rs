//! Types for database components that implement the revm `Database` trait.
#![warn(missing_docs)]

use edr_blockchain_api::BlockHashByNumber;
use edr_primitives::{Address, Bytecode, HashMap, B256, U256};
use edr_state_api::{
    account::{Account, AccountInfo},
    State, StateCommit,
};
use revm_database_interface::{DBErrorMarker, DatabaseRef};
pub use revm_database_interface::{Database, WrapDatabaseRef};

/// Wrapper type around a blockchain and state to implement the `Database`
/// trait.
pub struct DatabaseComponents<BlockchainT, StateT> {
    /// The blockchain component.
    pub blockchain: BlockchainT,
    /// The state component.
    pub state: StateT,
}

/// Wrapper type around a blockchain and state error.
#[derive(Debug, thiserror::Error)]
pub enum DatabaseComponentError<BlockchainErrorT, StateErrorT> {
    /// Error caused by the blockchain.
    #[error(transparent)]
    Blockchain(BlockchainErrorT),
    /// Error caused by the state.
    #[error(transparent)]
    State(StateErrorT),
}

impl<BlockchainErrorT, StateErrorT> DBErrorMarker
    for DatabaseComponentError<BlockchainErrorT, StateErrorT>
{
}

impl<BlockchainT, StateT> DatabaseRef for DatabaseComponents<BlockchainT, StateT>
where
    BlockchainT: BlockHashByNumber<Error: std::error::Error>,
    StateT: State<Error: std::error::Error>,
{
    type Error = DatabaseComponentError<BlockchainT::Error, StateT::Error>;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.state.basic(address).map_err(Self::Error::State)
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.state
            .code_by_hash(code_hash)
            .map_err(Self::Error::State)
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.state
            .storage(address, index)
            .map_err(Self::Error::State)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        self.blockchain
            .block_hash_by_number(number)
            .map_err(Self::Error::Blockchain)
    }
}

impl<BlockchainT: BlockHashByNumber, StateT: StateCommit> StateCommit
    for DatabaseComponents<BlockchainT, StateT>
{
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        self.state.commit(changes);
    }
}
