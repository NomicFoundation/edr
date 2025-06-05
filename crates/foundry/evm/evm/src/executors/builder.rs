use foundry_evm_core::{
    backend::Backend,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, EvmEnv, HardforkTr, TransactionEnvTr,
    },
    fork::CreateFork,
};
use revm::context::result::HaltReasonTr;

use crate::{executors::Executor, inspectors::InspectorStackBuilder};

/// The builder that allows to configure an evm [`Executor`] which a stack of
/// optional [`revm::Inspector`]s, such as [`Cheatcodes`].
///
/// By default, the [`Executor`] will be configured with an empty
/// [`InspectorStack`].
///
/// [`Cheatcodes`]: super::inspector::Cheatcodes
/// [`InspectorStack`]: super::inspector::InspectorStack
#[derive(Clone, Debug)]
#[must_use = "builders do nothing unless you call `build` on them"]
pub struct ExecutorBuilder<BlockT, TxT, HardforkT, ChainContextT>
where
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    HardforkT: HardforkTr,
    ChainContextT: ChainContextTr,
{
    /// The configuration used to build an [`InspectorStack`].
    stack: InspectorStackBuilder<BlockT, TxT, HardforkT, ChainContextT>,
    /// The gas limit.
    gas_limit: Option<u64>,
    /// The spec ID.
    spec_id: HardforkT,
    /// The fork to use at launch
    fork: Option<CreateFork<BlockT, TxT, HardforkT>>,
    /// The configured evm
    env: EvmEnv<BlockT, TxT, HardforkT>,
    /// The chain context
    chain_context: ChainContextT,
}

impl<BlockT, TxT, HardforkT, ChainContextT> Default
    for ExecutorBuilder<BlockT, TxT, HardforkT, ChainContextT>
where
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    HardforkT: HardforkTr,
    ChainContextT: ChainContextTr,
{
    #[inline]
    fn default() -> Self {
        Self {
            stack: InspectorStackBuilder::new(),
            gas_limit: None,
            spec_id: HardforkT::default(),
            fork: None,
            env: EvmEnv::default(),
            chain_context: ChainContextT::default(),
        }
    }
}

impl<BlockT, TxT, HardforkT, ChainContextT> ExecutorBuilder<BlockT, TxT, HardforkT, ChainContextT>
where
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    HardforkT: HardforkTr + Default,
    ChainContextT: ChainContextTr + Default,
{
    /// Create a new executor builder.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Modify the inspector stack.
    #[inline]
    pub fn inspectors(
        mut self,
        f: impl FnOnce(
            InspectorStackBuilder<BlockT, TxT, HardforkT, ChainContextT>,
        ) -> InspectorStackBuilder<BlockT, TxT, HardforkT, ChainContextT>,
    ) -> Self {
        self.stack = f(self.stack);
        self
    }

    /// Set the env
    #[inline]
    pub fn env(mut self, env: EvmEnv<BlockT, TxT, HardforkT>) -> Self {
        self.env = env;
        self
    }

    /// Set the chain context
    #[inline]
    pub fn chain_context(mut self, chain_context: ChainContextT) -> Self {
        self.chain_context = chain_context;
        self
    }

    /// Set the fork
    #[inline]
    pub fn fork(mut self, fork: Option<CreateFork<BlockT, TxT, HardforkT>>) -> Self {
        self.fork = fork;
        self
    }

    /// Sets the executor gas limit.
    ///
    /// See [`Executor::gas_limit`] for more info on why you might want to set
    /// this.
    #[inline]
    pub fn gas_limit(mut self, gas_limit: u64) -> Self {
        self.gas_limit = Some(gas_limit);
        self
    }

    /// Sets the EVM spec to use
    #[inline]
    pub fn spec(mut self, spec: HardforkT) -> Self {
        self.spec_id = spec;
        self
    }

    /// Builds the executor as configured.
    pub fn build<
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TxT>,
        HaltReasonT: HaltReasonTr,
    >(
        self,
    ) -> Executor<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, ChainContextT> {
        let Self {
            mut stack,
            gas_limit,
            spec_id,
            fork,
            mut env,
            chain_context,
        } = self;

        stack.block = Some(env.block.clone().into());
        stack.gas_price = Some(env.tx.gas_price());

        env.cfg.spec = spec_id;

        let gas_limit = gas_limit.unwrap_or(env.block.gas_limit());

        Executor::new(
            Backend::spawn(fork),
            env,
            chain_context,
            stack.build(),
            gas_limit,
        )
    }
}
