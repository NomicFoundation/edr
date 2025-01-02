use edr_eth::spec::ChainSpec;
use revm::JournaledState;
pub use revm::{
    handler,
    interpreter,
    // wiring::{evm_wiring::EvmWiring as PrimitiveEvmWiring, result},
    Context,
    // ContextPrecompile, EvmContext, EvmWiring, FrameOrResult, FrameResult, InnerEvmContext,
    JournalEntry,
};

use crate::{
    blockchain::BlockHash,
    config::CfgEnv,
    result::EVMErrorForChain,
    state::{DatabaseComponents, State},
};

pub type EvmForChainSpec<BlockchainT, ChainSpecT, StateT> = revm::Evm<
    EVMErrorForChain<ChainSpecT, <BlockchainT as BlockHash>::Error, <StateT as State>::Error>,
    EvmContextForChainSpec<BlockchainT, ChainSpecT, StateT>,
    // TODO: Custom handler
>;

pub type EvmContextForChainSpec<BlockchainT, ChainSpecT, StateT> = revm::Context<
    <ChainSpecT as ChainSpec>::BlockEnv,
    <ChainSpecT as ChainSpec>::SignedTransaction,
    CfgEnv,
    DatabaseComponents<BlockchainT, StateT>,
    JournaledState<DatabaseComponents<BlockchainT, StateT>>,
    <ChainSpecT as ChainSpec>::Context,
>;
