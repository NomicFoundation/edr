use edr_evm_spec::{ChainSpec, TransactionValidation};
use edr_state_api::database::DatabaseComponentError;
pub use revm_context_interface::result::{
    EVMError, ExecutionResult, Output, ResultAndState as ExecutionResultAndState, ResultAndState,
    SuccessReason,
};

/// EVM error type for a specific chain.
pub type EVMErrorForChain<ChainSpecT, BlockChainErrorT, StateErrorT> = EVMError<
    DatabaseComponentError<BlockChainErrorT, StateErrorT>,
    <<ChainSpecT as ChainSpec>::SignedTransaction as TransactionValidation>::ValidationError,
>;
