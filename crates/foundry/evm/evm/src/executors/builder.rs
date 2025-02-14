use alloy_primitives::U256;
use foundry_evm_core::{
    backend::Backend,
    fork::{CreateFork, MultiFork},
};
use revm::primitives::{Env, EnvWithHandlerCfg, SpecId};

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
pub struct ExecutorBuilder {
    /// The configuration used to build an [`InspectorStack`].
    stack: InspectorStackBuilder,
    /// The gas limit.
    gas_limit: Option<U256>,
    /// The spec ID.
    spec_id: SpecId,
    /// The fork to use at launch
    fork: Option<CreateFork>,
    /// The configured evm
    env: Env,
    /// The forks to use in the backend.
    multi_fork: Option<MultiFork>,
}

impl Default for ExecutorBuilder {
    #[inline]
    fn default() -> Self {
        Self {
            stack: InspectorStackBuilder::new(),
            gas_limit: None,
            spec_id: SpecId::LATEST,
            fork: None,
            env: Env::default(),
            multi_fork: None,
        }
    }
}

impl ExecutorBuilder {
    /// Create a new executor builder.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Modify the inspector stack.
    #[inline]
    pub fn inspectors(
        mut self,
        f: impl FnOnce(InspectorStackBuilder) -> InspectorStackBuilder,
    ) -> Self {
        self.stack = f(self.stack);
        self
    }

    /// Set the env
    #[inline]
    pub fn env(mut self, env: Env) -> Self {
        self.env = env;
        self
    }

    /// Set the fork
    #[inline]
    pub fn fork(mut self, fork: Option<CreateFork>) -> Self {
        self.fork = fork;
        self
    }

    /// Sets the executor gas limit.
    ///
    /// See [`Executor::gas_limit`] for more info on why you might want to set
    /// this.
    #[inline]
    pub fn gas_limit(mut self, gas_limit: U256) -> Self {
        self.gas_limit = Some(gas_limit);
        self
    }

    /// Sets the multi fork to use from a previous execution
    #[inline]
    pub fn multi_fork(mut self, multi_fork: MultiFork) -> Self {
        self.multi_fork = Some(multi_fork);
        self
    }

    /// Sets the EVM spec to use
    #[inline]
    pub fn spec(mut self, spec: SpecId) -> Self {
        self.spec_id = spec;
        self
    }

    /// Builds the executor as configured.
    pub fn build(self) -> Executor {
        let Self {
            mut stack,
            gas_limit,
            spec_id,
            fork,
            env,
            multi_fork,
        } = self;
        stack.block = Some(env.block.clone());
        stack.gas_price = Some(env.tx.gas_price);
        let gas_limit = gas_limit.unwrap_or(env.block.gas_limit);

        let backend = if let Some(multi_fork) = multi_fork {
            Backend::new(multi_fork, fork)
        } else {
            Backend::spawn(fork)
        };
        Executor::new(
            backend,
            EnvWithHandlerCfg::new_with_spec_id(Box::new(env), spec_id),
            stack.build(),
            gas_limit,
        )
    }
}
