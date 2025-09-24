use edr_evm_spec::{ChainSpec, TransactionValidation};
pub use revm_context_interface::result::{
    EVMError, ExecutionResult, Output, ResultAndState as ExecutionResultAndState, ResultAndState,
    SuccessReason,
};

use crate::state::DatabaseComponentError;

/// EVM error type for a specific chain.
pub type EVMErrorForChain<ChainSpecT, BlockChainErrorT, StateErrorT> = EVMError<
    DatabaseComponentError<BlockChainErrorT, StateErrorT>,
    <<ChainSpecT as ChainSpec>::SignedTransaction as TransactionValidation>::ValidationError,
>;
