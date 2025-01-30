use edr_eth::{Address, Bytes, HashMap, HashSet};
pub use revm::precompile::{u64_to_address, PrecompileFn, PrecompileSpecId, Precompiles};
use revm::{
    handler_interface::PrecompileProvider,
    interpreter::{Gas, InstructionResult, InterpreterResult},
    precompile::PrecompileErrors,
};

use crate::{evm::EvmSpec, spec::RuntimeSpec};

/// Helper type for a chain-specific overridden precompile provider.
pub type OverriddenPrecompileProviderForChainSpec<
    BlockchainErrorT,
    ChainSpecT,
    ContextT,
    StateErrorT,
> = OverriddenPrecompileProvider<
    <<ChainSpecT as RuntimeSpec>::Evm<BlockchainErrorT, ContextT, StateErrorT> as EvmSpec<
        BlockchainErrorT,
        ChainSpecT,
        ContextT,
        StateErrorT,
    >>::PrecompileProvider,
>;

/// A precompile provider that allows adding custom or overwriting existing
/// precompiles.
///
/// # Safety
///
/// This assumes that the base precompile provider does not change its
/// precompiles after construction.
#[derive(Clone)]
pub struct OverriddenPrecompileProvider<BaseProviderT: PrecompileProvider> {
    base: BaseProviderT,
    custom_precompiles: HashMap<Address, PrecompileFn>,
    // Cache of unique addresses to avoid reporting duplicates between `base` and
    // `custom_precompiles`. This speeds up the `warm_addresses` method.
    unique_addresses: HashSet<Address>,
}

impl<BaseProviderT: PrecompileProvider> OverriddenPrecompileProvider<BaseProviderT> {
    /// Creates a new custom precompile provider.
    pub fn new(base: BaseProviderT) -> Self {
        Self::with_precompiles(base, HashMap::new())
    }

    /// Creates a new custom precompile provider with custom precompiles.
    pub fn with_precompiles(
        base: BaseProviderT,
        custom_precompiles: HashMap<Address, PrecompileFn>,
    ) -> Self {
        let unique_addresses = custom_precompiles
            .keys()
            .cloned()
            .chain(base.warm_addresses())
            .collect();

        Self {
            base,
            custom_precompiles,
            unique_addresses,
        }
    }

    /// Adds a custom precompile.
    pub fn set_precompile(&mut self, address: Address, precompile: PrecompileFn) {
        self.custom_precompiles.insert(address, precompile);
        self.unique_addresses.insert(address);
    }
}

/// Trait for providing custom precompiles.
pub trait CustomPrecompilesGetter {
    /// Returns a map of custom precompiles.
    fn custom_precompiles(&self) -> HashMap<Address, PrecompileFn>;
}

/// A context that contains custom precompiles.
pub struct ContextWithCustomPrecompiles<ContextT> {
    /// The type of the context.
    pub context: ContextT,
    /// The custom precompiles.
    pub custom_precompiles: HashMap<Address, PrecompileFn>,
}

impl<ContextT> CustomPrecompilesGetter for ContextWithCustomPrecompiles<ContextT> {
    fn custom_precompiles(&self) -> HashMap<Address, PrecompileFn> {
        self.custom_precompiles.clone()
    }
}

impl<BaseProviderT, ContextT, ErrorT> PrecompileProvider
    for OverriddenPrecompileProvider<BaseProviderT>
where
    BaseProviderT:
        PrecompileProvider<Context = ContextT, Error = ErrorT, Output = InterpreterResult>,
    ContextT: CustomPrecompilesGetter,
    ErrorT: From<PrecompileErrors>,
{
    type Context = ContextT;
    type Error = ErrorT;
    type Output = InterpreterResult;

    fn new(context: &mut Self::Context) -> Self {
        let base = BaseProviderT::new(context);
        let custom_precompiles = context.custom_precompiles();

        Self::with_precompiles(base, custom_precompiles)
    }

    fn run(
        &mut self,
        context: &mut Self::Context,
        address: &Address,
        bytes: &edr_eth::Bytes,
        gas_limit: u64,
    ) -> Result<Option<Self::Output>, Self::Error> {
        let Some(precompile) = self.custom_precompiles.get(address) else {
            return self.base.run(context, address, bytes, gas_limit);
        };

        let mut result = InterpreterResult {
            result: InstructionResult::Return,
            gas: Gas::new(gas_limit),
            output: Bytes::new(),
        };

        match (*precompile)(bytes, gas_limit) {
            Ok(output) => {
                let underflow = result.gas.record_cost(output.gas_used);
                assert!(underflow, "Gas underflow is not possible");
                result.result = InstructionResult::Return;
                result.output = output.bytes;
            }
            Err(PrecompileErrors::Error(e)) => {
                result.result = if e.is_oog() {
                    InstructionResult::PrecompileOOG
                } else {
                    InstructionResult::PrecompileError
                };
            }
            Err(err @ PrecompileErrors::Fatal { .. }) => return Err(err.into()),
        }
        Ok(Some(result))
    }

    fn warm_addresses(&self) -> impl Iterator<Item = Address> {
        self.unique_addresses.iter().cloned()
    }

    fn contains(&self, address: &Address) -> bool {
        self.unique_addresses.contains(address)
    }
}
