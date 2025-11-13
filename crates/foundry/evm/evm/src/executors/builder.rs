use foundry_evm_core::{
    backend::{Backend, Predeploy},
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, EvmEnv, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
    fork::CreateFork,
};
use revm::context::result::HaltReasonTr;

use crate::{executors::Executor, inspectors::InspectorStackBuilder};

#[derive(Clone, Debug)]
#[derive(thiserror::Error)]
pub enum ExecutorBuilderError {
    #[error("Failed to create backend: {0}")]
    BackendError(String)
}

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
    stack: InspectorStackBuilder<HardforkT, ChainContextT>,
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
    /// The predeploys for the chain
    local_predeploys: Vec<Predeploy>,
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
            local_predeploys: Vec::default(),
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
            InspectorStackBuilder<HardforkT, ChainContextT>,
        ) -> InspectorStackBuilder<HardforkT, ChainContextT>,
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

    /// The predeploys applied in local mode.
    /// These should match the predeploys of the network in fork mode, so they
    /// aren't set in fork mode.
    #[inline]
    pub fn local_predeploys(mut self, local_predeploys: Vec<Predeploy>) -> Self {
        self.local_predeploys = local_predeploys;
        self
    }

    /// Sets the EVM spec to use
    #[inline]
    pub fn spec(mut self, spec: HardforkT) -> Self {
        self.spec_id = spec;
        self
    }

    /// Builds the executor as configured.
    #[allow(clippy::type_complexity)]
    pub fn build<
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        TransactionErrorT: TransactionErrorTrait,
    >(
        self,
    ) -> Result<Executor<BlockT, TxT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT, ChainContextT>, ExecutorBuilderError>
    {
        let Self {
            mut stack,
            gas_limit,
            spec_id,
            fork,
            mut env,
            chain_context,
            local_predeploys,
        } = self;

        if stack.block.is_none() {
            stack.block = Some(env.block.clone().into());
        }
        if stack.gas_price.is_none() {
            stack.gas_price = Some(env.tx.gas_price());
        }

        env.cfg.spec = spec_id;

        let gas_limit = gas_limit.unwrap_or(env.block.gas_limit());

        let backend = Backend::spawn(fork, local_predeploys).map_err(|err| ExecutorBuilderError::BackendError(err.to_string()))?;

        Ok(Executor::new(
            backend,
            env,
            chain_context,
            stack.build(),
            gas_limit,
        ))
    }
}
