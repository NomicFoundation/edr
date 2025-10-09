use edr_chain_spec::{ChainSpec, TransactionValidation};
use edr_database_components::DatabaseComponentError;
pub use revm_context_interface::result::{
    EVMError, ExecutionResult, Output, ResultAndState as ExecutionResultAndState, ResultAndState,
    SuccessReason,
};

/// EVM error type for a specific chain.
pub type EVMErrorForChain<ChainSpecT, BlockChainErrorT, StateErrorT> = EVMError<
    DatabaseComponentError<BlockChainErrorT, StateErrorT>,
    <<ChainSpecT as ChainSpec>::SignedTransaction as TransactionValidation>::ValidationError,
>;
