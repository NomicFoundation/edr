//! Types for an EVM execution handler.

use revm_context::{Cfg, ContextTr};
use revm_handler::PrecompileProvider;
pub use revm_handler::{instructions::EthInstructions, EthFrame, EthPrecompiles};
use revm_interpreter::{CallInputs, InterpreterResult};
use revm_primitives::{hardfork::SpecId, Address};

/// Wrapper around [`EthPrecompiles`] that implements [`Default`].
///
/// In `revm` 38, [`EthPrecompiles`] no longer implements [`Default`] and must
/// be constructed via [`EthPrecompiles::new`] with a [`SpecId`]. EDR's chain
/// spec trait requires the precompile provider to be `Default`-constructible
/// so that callers can build it ahead of knowing the hardfork and later refine
/// the spec via [`PrecompileProvider::set_spec`]. This newtype provides a
/// [`Default`] impl that initializes with [`SpecId::default()`]; the spec is
/// overwritten on the first [`PrecompileProvider::set_spec`] call, so the
/// placeholder is inconsequential.
#[derive(Clone, Debug)]
pub struct DefaultEthPrecompiles(pub EthPrecompiles);

impl Default for DefaultEthPrecompiles {
    fn default() -> Self {
        Self(EthPrecompiles::new(SpecId::default()))
    }
}

impl<CTX: ContextTr> PrecompileProvider<CTX> for DefaultEthPrecompiles
where
    EthPrecompiles: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: <CTX::Cfg as Cfg>::Spec) -> bool {
        self.0.set_spec(spec)
    }

    fn run(
        &mut self,
        context: &mut CTX,
        inputs: &CallInputs,
    ) -> Result<Option<Self::Output>, String> {
        self.0.run(context, inputs)
    }

    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        Box::new(<EthPrecompiles as PrecompileProvider<CTX>>::warm_addresses(
            &self.0,
        ))
    }

    fn contains(&self, address: &Address) -> bool {
        <EthPrecompiles as PrecompileProvider<CTX>>::contains(&self.0, address)
    }
}
