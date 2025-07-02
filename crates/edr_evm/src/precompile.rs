use std::marker::PhantomData;

use edr_eth::{Address, Bytes, HashMap, HashSet};
pub use revm_handler::{EthPrecompiles, PrecompileProvider};
use revm_interpreter::{Gas, InstructionResult, InterpreterResult};
pub use revm_precompile::{
    secp256r1, u64_to_address, PrecompileError, PrecompileFn, PrecompileSpecId,
    PrecompileWithAddress, Precompiles,
};

use crate::{config::Cfg, interpreter::InputsImpl, spec::ContextTrait};

/// A precompile provider that allows adding custom or overwriting existing
/// precompiles.
#[derive(Clone)]
pub struct OverriddenPrecompileProvider<
    BaseProviderT: PrecompileProvider<ContextT, Output = InterpreterResult>,
    ContextT: ContextTrait,
> {
    base: BaseProviderT,
    custom_precompiles: HashMap<Address, PrecompileFn>,
    // Cache of unique addresses to avoid reporting duplicates between `base` and
    // `custom_precompiles`. This speeds up the `warm_addresses` method.
    unique_addresses: HashSet<Address>,
    phantom: PhantomData<ContextT>,
}

impl<
        BaseProviderT: PrecompileProvider<ContextT, Output = InterpreterResult>,
        ContextT: ContextTrait,
    > OverriddenPrecompileProvider<BaseProviderT, ContextT>
{
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
            phantom: PhantomData,
        }
    }

    /// Adds a custom precompile.
    pub fn set_precompile(&mut self, address: Address, precompile: PrecompileFn) {
        self.custom_precompiles.insert(address, precompile);
        self.unique_addresses.insert(address);
    }
}

impl<
        BaseProviderT: PrecompileProvider<ContextT, Output = InterpreterResult>,
        ContextT: ContextTrait,
    > PrecompileProvider<ContextT> for OverriddenPrecompileProvider<BaseProviderT, ContextT>
{
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: <ContextT::Cfg as Cfg>::Spec) -> bool {
        self.base.set_spec(spec);

        // Update unique addresses
        self.unique_addresses = self
            .custom_precompiles
            .keys()
            .cloned()
            .chain(self.base.warm_addresses())
            .collect();

        true
    }

    fn run(
        &mut self,
        context: &mut ContextT,
        address: &Address,
        inputs: &InputsImpl,
        is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<Self::Output>, String> {
        let Some(precompile) = self.custom_precompiles.get(address) else {
            return self
                .base
                .run(context, address, inputs, is_static, gas_limit);
        };

        let mut result = InterpreterResult {
            result: InstructionResult::Return,
            gas: Gas::new(gas_limit),
            output: Bytes::new(),
        };

        let input_data = &inputs.input.bytes(context);

        match (*precompile)(input_data, gas_limit) {
            Ok(output) => {
                let underflow = result.gas.record_cost(output.gas_used);
                assert!(underflow, "Gas underflow is not possible");
                result.result = InstructionResult::Return;
                result.output = output.bytes;
            }
            Err(PrecompileError::Fatal(e)) => return Err(e),
            Err(e) => {
                result.result = if e.is_oog() {
                    InstructionResult::PrecompileOOG
                } else {
                    InstructionResult::PrecompileError
                };
            }
        }
        Ok(Some(result))
    }

    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        Box::new(self.unique_addresses.iter().cloned())
    }

    fn contains(&self, address: &Address) -> bool {
        self.unique_addresses.contains(address)
    }
}
