// Re-export `edr_eth` types.
pub use edr_eth::result::*;
use edr_eth::{spec::ChainSpec, transaction::TransactionValidation};
pub use revm_context_interface::result::EVMError;

use crate::{precompile::PrecompileError, state::DatabaseComponentError};

/// EVM error type for a specific chain.
pub type EVMErrorForChain<ChainSpecT, BlockChainErrorT, StateErrorT> = EVMError<
    DatabaseComponentError<BlockChainErrorT, StateErrorT>,
    PrecompileError,
    <<ChainSpecT as ChainSpec>::SignedTransaction as TransactionValidation>::ValidationError,
>;
