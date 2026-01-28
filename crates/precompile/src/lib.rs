//! Types for EVM precompiles.
#![warn(missing_docs)]

use std::marker::PhantomData;

use edr_primitives::{Address, Bytes, HashMap, HashSet};
use revm_context_interface::{Cfg, ContextTr as ContextTrait, JournalTr as _, LocalContextTr as _};
pub use revm_handler::{EthPrecompiles, PrecompileProvider};
use revm_interpreter::{CallInput, CallInputs, Gas, InstructionResult, InterpreterResult};
pub use revm_precompile::{
    secp256r1, u64_to_address, Precompile, PrecompileError, PrecompileFn, PrecompileSpecId,
    Precompiles,
};

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
        Self::with_precompiles(base, HashMap::default())
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

    /// Consumes the provider and returns the set of all unique precompile
    /// addresses.
    pub fn into_addresses(self) -> HashSet<Address> {
        self.unique_addresses
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
        inputs: &CallInputs,
    ) -> Result<Option<Self::Output>, String> {
        let Some(precompile) = self.custom_precompiles.get(&inputs.bytecode_address) else {
            return self.base.run(context, inputs);
        };

        let mut result = InterpreterResult {
            result: InstructionResult::Return,
            gas: Gas::new(inputs.gas_limit),
            output: Bytes::new(),
        };

        let exec_result = {
            let r;
            let input_bytes = match &inputs.input {
                CallInput::SharedBuffer(range) => {
                    if let Some(slice) = context.local().shared_memory_buffer_slice(range.clone()) {
                        r = slice;
                        r.as_ref()
                    } else {
                        &[]
                    }
                }
                CallInput::Bytes(bytes) => bytes.0.iter().as_slice(),
            };
            (*precompile)(input_bytes, inputs.gas_limit)
        };

        match exec_result {
            Ok(output) => {
                let underflow = result.gas.record_cost(output.gas_used);
                assert!(underflow, "Gas underflow is not possible");
                result.result = if output.reverted {
                    InstructionResult::Revert
                } else {
                    InstructionResult::Return
                };
                result.output = output.bytes;
            }
            Err(PrecompileError::Fatal(e)) => return Err(e),
            Err(e) => {
                result.result = if e.is_oog() {
                    InstructionResult::PrecompileOOG
                } else {
                    InstructionResult::PrecompileError
                };
                // If this is a top-level precompile call (depth == 1), persist the error
                // message into the local context so it can be returned as
                // output in the final result. Only do this for non-OOG errors
                // (OOG is a distinct halt reason without output).
                if !e.is_oog() && context.journal().depth() == 1 {
                    context
                        .local_mut()
                        .set_precompile_error_context(e.to_string());
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_364_kzg_point_evaluation_present_in_cancun() {
        const KZG_POINT_EVALUATION_ADDRESS: Address = u64_to_address(0x0A);

        let precompiles = Precompiles::cancun();
        assert!(precompiles.contains(&KZG_POINT_EVALUATION_ADDRESS));
    }
}
