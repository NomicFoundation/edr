use std::fmt::Debug;

use edr_evm::{
    db::Database,
    evm::EvmHandler,
    register_eip_3155_and_raw_tracers_handles,
    trace::{register_trace_collector_handles, TraceCollector},
    GetContextData, TracerEip3155,
};

use crate::{
    console_log::ConsoleLogCollector, debugger::register_debugger_handles, mock::Mocker,
    precompiles::register_precompiles_handles,
};

/// Registers debugger and precompile handles.
pub fn register_debugger_and_precompile<const ENABLE_RIP_7212: bool, DatabaseT, ContextT>(
    handler: &mut EvmHandler<'_, ContextT, DatabaseT>,
) where
    DatabaseT: Database,
    DatabaseT::Error: Debug,
    ContextT: GetContextData<ConsoleLogCollector>
        + GetContextData<Mocker>
        + GetContextData<TraceCollector>,
{
    register_debugger_handles(handler);
    register_precompiles_handles::<ENABLE_RIP_7212, _, _>(handler);
}

pub fn register_eip_3155_and_raw_tracers_and_precompile<
    const ENABLE_RIP_7212: bool,
    DatabaseT,
    ContextT,
>(
    handler: &mut EvmHandler<'_, ContextT, DatabaseT>,
) where
    DatabaseT: Database,
    DatabaseT::Error: Debug,
    ContextT: GetContextData<TraceCollector> + GetContextData<TracerEip3155>,
{
    register_eip_3155_and_raw_tracers_handles(handler);
    register_precompiles_handles::<ENABLE_RIP_7212, _, _>(handler);
}

pub fn register_trace_collector_and_precompile<const ENABLE_RIP_7212: bool, DatabaseT, ContextT>(
    handler: &mut EvmHandler<'_, ContextT, DatabaseT>,
) where
    DatabaseT: Database,
    DatabaseT::Error: Debug,
    ContextT: GetContextData<TraceCollector>,
{
    register_trace_collector_handles(handler);
    register_precompiles_handles::<ENABLE_RIP_7212, _, _>(handler);
}
