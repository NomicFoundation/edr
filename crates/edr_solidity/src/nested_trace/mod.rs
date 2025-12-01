//! Naive Rust port of the `MessageTrace` et al. from Hardhat.

mod conversion;

use std::{collections::HashMap, sync::Arc};

use derive_where::derive_where;
use edr_chain_spec::HaltReasonTrait;
use edr_primitives::{Address, Bytes, U256};

use crate::{build_model::ContractMetadata, exit_code::ExitCode};

/// An EVM trace where the steps are nested according to the call stack.
#[derive(Clone, Debug)]
pub enum NestedTrace<HaltReasonT: HaltReasonTrait> {
    /// Represents a create trace.
    Create(CreateMessage<HaltReasonT>),
    /// Represents a call trace.
    Call(CallMessage<HaltReasonT>),
    /// Represents a precompile trace.
    Precompile(PrecompileMessage<HaltReasonT>),
}

impl<HaltReasonT: HaltReasonTrait> NestedTrace<HaltReasonT> {
    /// Returns the exit code of the trace.
    pub fn exit_code(&self) -> &ExitCode<HaltReasonT> {
        match self {
            Self::Create(create) => &create.exit,
            Self::Call(call) => &call.exit,
            Self::Precompile(precompile) => &precompile.exit,
        }
    }

    /// Converts a `CallTraceArena` from `revm_inspectors` into a `NestedTrace`.
    ///
    /// This function bridges the gap between the halt-reason-agnostic `TracingInspector` and the
    /// halt-reason-aware `NestedTrace` format.
    ///
    /// # Arguments
    ///
    /// * `address_to_creation_code` - Mapping from contract addresses to their creation code
    /// * `address_to_runtime_code` - Mapping from contract addresses to their runtime code
    /// * `arena` - The call trace arena to convert
    ///
    /// # Errors
    ///
    /// Returns an error if the arena is empty or has an invalid root node.
    pub fn from_call_trace_arena(
        address_to_creation_code: &HashMap<Address, &Bytes>,
        address_to_runtime_code: &HashMap<Address, &Bytes>,
        arena: &revm_inspectors::tracing::CallTraceArena,
    ) -> Result<Self, conversion::TraceConversionError> {
        conversion::convert_from_arena(address_to_creation_code, address_to_runtime_code, arena)
    }
}

/// Represents a precompile message.
#[derive(Clone, Debug)]
pub struct PrecompileMessage<HaltReasonT> {
    /// Precompile number.
    pub precompile: u32,
    /// Calldata buffer
    pub calldata: Bytes,
    /// Value of the message.
    pub value: U256,
    /// Return data buffer.
    pub return_data: Bytes,
    /// EVM exit code.
    pub exit: ExitCode<HaltReasonT>,
    /// How much gas was used.
    pub gas_used: u64,
    /// Depth of the message.
    pub depth: usize,
}

impl<HaltReasonT> PrecompileMessage<HaltReasonT> {
    /// Converts the type of the halt reason of the instance.
    pub fn map_halt_reason<ConversionFnT: Fn(HaltReasonT) -> NewHaltReasonT, NewHaltReasonT>(
        self,
        conversion_fn: ConversionFnT,
    ) -> PrecompileMessage<NewHaltReasonT> {
        PrecompileMessage {
            precompile: self.precompile,
            calldata: self.calldata,
            value: self.value,
            return_data: self.return_data,
            exit: self.exit.map_halt_reason(conversion_fn),
            gas_used: self.gas_used,
            depth: self.depth,
        }
    }
}

/// Represents a create message.
#[derive(Clone, Debug)]
pub struct CreateMessage<HaltReasonT> {
    // The following is just an optimization: When processing this traces it's useful to know ahead
    // of time how many subtraces there are.
    /// Number of subtraces. Used to speed up the processing of the traces in
    /// JS.
    pub number_of_subtraces: u32,
    /// Children messages.
    pub steps: Vec<NestedTraceStep<HaltReasonT>>,
    /// Resolved metadata of the contract that is being executed.
    pub contract_meta: Option<Arc<ContractMetadata>>,
    /// Address of the deployed contract.
    pub deployed_contract: Option<Bytes>,
    /// Code of the contract that is being executed.
    pub code: Bytes,
    /// Value of the message.
    pub value: U256,
    /// Return data buffer.
    pub return_data: Bytes,
    /// EVM exit code.
    pub exit: ExitCode<HaltReasonT>,
    /// How much gas was used.
    pub gas_used: u64,
    /// Depth of the message.
    pub depth: usize,
}

impl<HaltReasonT> CreateMessage<HaltReasonT> {
    /// Converts the type of the halt reason of the instance.
    pub fn map_halt_reason<
        ConversionFnT: Copy + Fn(HaltReasonT) -> NewHaltReasonT,
        NewHaltReasonT,
    >(
        self,
        conversion_fn: ConversionFnT,
    ) -> CreateMessage<NewHaltReasonT> {
        CreateMessage {
            number_of_subtraces: self.number_of_subtraces,
            steps: self
                .steps
                .into_iter()
                .map(|step| step.map_halt_reason(conversion_fn))
                .collect(),
            contract_meta: self.contract_meta,
            deployed_contract: self.deployed_contract,
            code: self.code,
            value: self.value,
            return_data: self.return_data,
            exit: self.exit.map_halt_reason(conversion_fn),
            gas_used: self.gas_used,
            depth: self.depth,
        }
    }
}

/// Represents a call message with contract metadata.
#[derive(Clone, Debug)]
pub struct CallMessage<HaltReasonT> {
    // The following is just an optimization: When processing this traces it's useful to know ahead
    // of time how many subtraces there are.
    /// Number of subtraces. Used to speed up the processing of the traces in
    /// JS.
    pub number_of_subtraces: u32,
    /// Children messages.
    pub steps: Vec<NestedTraceStep<HaltReasonT>>,
    /// Resolved metadata of the contract that is being executed.
    pub contract_meta: Option<Arc<ContractMetadata>>,
    /// Calldata buffer
    pub calldata: Bytes,
    /// Address of the contract that is being executed.
    pub address: Address,
    /// Address of the code that is being executed.
    pub code_address: Address,
    /// Code of the contract that is being executed.
    pub code: Bytes,
    /// Value of the message.
    pub value: U256,
    /// Return data buffer.
    pub return_data: Bytes,
    /// EVM exit code.
    pub exit: ExitCode<HaltReasonT>,
    /// How much gas was used.
    pub gas_used: u64,
    /// Depth of the message.
    pub depth: usize,
}

impl<HaltReasonT> CallMessage<HaltReasonT> {
    /// Converts the type of the halt reason of the instance.
    pub fn map_halt_reason<
        ConversionFnT: Copy + Fn(HaltReasonT) -> NewHaltReasonT,
        NewHaltReasonT,
    >(
        self,
        conversion_fn: ConversionFnT,
    ) -> CallMessage<NewHaltReasonT> {
        CallMessage {
            number_of_subtraces: self.number_of_subtraces,
            steps: self
                .steps
                .into_iter()
                .map(|step| step.map_halt_reason(conversion_fn))
                .collect(),
            contract_meta: self.contract_meta,
            calldata: self.calldata,
            address: self.address,
            code_address: self.code_address,
            code: self.code,
            value: self.value,
            return_data: self.return_data,
            exit: self.exit.map_halt_reason(conversion_fn),
            gas_used: self.gas_used,
            depth: self.depth,
        }
    }
}

/// Represents a create or call message.
#[derive(Clone, Debug)]
pub enum CreateOrCallMessage<HaltReasonT: HaltReasonTrait> {
    /// Represents a create message.
    Create(CreateMessage<HaltReasonT>),
    /// Represents a call message.
    Call(CallMessage<HaltReasonT>),
}

impl<HaltReasonT: HaltReasonTrait> From<CreateMessage<HaltReasonT>>
    for CreateOrCallMessage<HaltReasonT>
{
    fn from(value: CreateMessage<HaltReasonT>) -> Self {
        CreateOrCallMessage::Create(value)
    }
}

impl<HaltReasonT: HaltReasonTrait> From<CallMessage<HaltReasonT>>
    for CreateOrCallMessage<HaltReasonT>
{
    fn from(value: CallMessage<HaltReasonT>) -> Self {
        CreateOrCallMessage::Call(value)
    }
}

/// Represents a create or call message.
#[derive(Debug)]
#[derive_where(Clone, Copy)]
pub(crate) enum CreateOrCallMessageRef<'a, HaltReasonT: HaltReasonTrait> {
    /// Represents a create message.
    Create(&'a CreateMessage<HaltReasonT>),
    /// Represents a call message.
    Call(&'a CallMessage<HaltReasonT>),
}

impl<'a, HaltReasonT: HaltReasonTrait> CreateOrCallMessageRef<'a, HaltReasonT> {
    pub fn contract_meta(&self) -> Option<Arc<ContractMetadata>> {
        match self {
            CreateOrCallMessageRef::Create(create) => create.contract_meta.as_ref().map(Arc::clone),
            CreateOrCallMessageRef::Call(call) => call.contract_meta.as_ref().map(Arc::clone),
        }
    }

    pub fn exit_code(&self) -> &ExitCode<HaltReasonT> {
        match self {
            CreateOrCallMessageRef::Create(create) => &create.exit,
            CreateOrCallMessageRef::Call(call) => &call.exit,
        }
    }

    pub fn number_of_subtraces(&self) -> u32 {
        match self {
            CreateOrCallMessageRef::Create(create) => create.number_of_subtraces,
            CreateOrCallMessageRef::Call(call) => call.number_of_subtraces,
        }
    }

    pub fn return_data(&self) -> &Bytes {
        match self {
            CreateOrCallMessageRef::Create(create) => &create.return_data,
            CreateOrCallMessageRef::Call(call) => &call.return_data,
        }
    }

    pub fn steps(&self) -> &'a [NestedTraceStep<HaltReasonT>] {
        match self {
            CreateOrCallMessageRef::Create(create) => create.steps.as_slice(),
            CreateOrCallMessageRef::Call(call) => call.steps.as_slice(),
        }
    }
}

impl<'a, HaltReasonT: HaltReasonTrait> From<&'a CreateOrCallMessage<HaltReasonT>>
    for CreateOrCallMessageRef<'a, HaltReasonT>
{
    fn from(value: &'a CreateOrCallMessage<HaltReasonT>) -> Self {
        match value {
            CreateOrCallMessage::Create(create) => CreateOrCallMessageRef::Create(create),
            CreateOrCallMessage::Call(call) => CreateOrCallMessageRef::Call(call),
        }
    }
}

impl<'a, HaltReasonT: HaltReasonTrait> From<&'a CreateMessage<HaltReasonT>>
    for CreateOrCallMessageRef<'a, HaltReasonT>
{
    fn from(value: &'a CreateMessage<HaltReasonT>) -> Self {
        CreateOrCallMessageRef::Create(value)
    }
}

impl<'a, HaltReasonT: HaltReasonTrait> From<&'a CallMessage<HaltReasonT>>
    for CreateOrCallMessageRef<'a, HaltReasonT>
{
    fn from(value: &'a CallMessage<HaltReasonT>) -> Self {
        CreateOrCallMessageRef::Call(value)
    }
}

/// Represents a nested trace step with contract metadata.
#[derive(Clone, Debug)]
pub enum NestedTraceStep<HaltReasonT> {
    /// Represents a create message.
    Create(CreateMessage<HaltReasonT>),
    /// Represents a call message.
    Call(CallMessage<HaltReasonT>),
    /// Represents a precompile message.
    Precompile(PrecompileMessage<HaltReasonT>),
    /// Minimal EVM step that contains only PC (program counter).
    Evm(EvmStep),
}

impl<HaltReasonT> NestedTraceStep<HaltReasonT> {
    /// Converts the type of the halt reason of the instance.
    pub fn map_halt_reason<
        ConversionFnT: Copy + Fn(HaltReasonT) -> NewHaltReasonT,
        NewHaltReasonT,
    >(
        self,
        conversion_fn: ConversionFnT,
    ) -> NestedTraceStep<NewHaltReasonT> {
        match self {
            NestedTraceStep::Create(create) => {
                NestedTraceStep::Create(create.map_halt_reason(conversion_fn))
            }
            NestedTraceStep::Call(call) => {
                NestedTraceStep::Call(call.map_halt_reason(conversion_fn))
            }
            NestedTraceStep::Precompile(precompile) => {
                NestedTraceStep::Precompile(precompile.map_halt_reason(conversion_fn))
            }
            NestedTraceStep::Evm(evm_step) => NestedTraceStep::Evm(evm_step),
        }
    }
}

/// Minimal EVM step that contains only PC (program counter).
#[derive(Clone, Debug)]
pub struct EvmStep {
    /// Program counter
    pub pc: u32,
}
