//! Gas reports.

use std::{collections::BTreeMap, ops::Deref};

use edr_common::calc;
use edr_gas_report::{
    ContractGasReport, DeploymentGasReport, FunctionGasReport, GasReportExecutionStatus,
};
use edr_solidity::{
    proxy_detection::{detect_proxy_chain, is_proxy_selector_and_output},
    solidity_stack_trace::UNRECOGNIZED_CONTRACT_NAME,
};
use foundry_evm::{abi::TestFunctionExt, traces::CallKind};
use serde::{Deserialize, Serialize};

use crate::{
    constants::{CHEATCODE_ADDRESS, HARDHAT_CONSOLE_ADDRESS},
    traces::{CallTraceArena, CallTraceDecoder, CallTraceNode, DecodedCallData},
};

/// Represents the gas report for a set of contracts.
#[derive(Clone, Debug, Default)]
pub struct GasReport {
    /// All contracts that were analyzed grouped by their identifier
    /// ``test/Counter.t.sol:CounterTest``
    pub contracts: BTreeMap<String, ContractInfo>,
}

impl GasReport {
    /// Analyzes the given traces and generates a gas report.
    pub async fn analyze(
        &mut self,
        arenas: impl IntoIterator<Item = &CallTraceArena>,
        decoder: &CallTraceDecoder,
    ) {
        for arena in arenas {
            for node in arena.nodes() {
                self.analyze_node(node, arena, decoder).await;
            }
        }
    }

    async fn analyze_node(
        &mut self,
        node: &CallTraceNode,
        arena: &CallTraceArena,
        decoder: &CallTraceDecoder,
    ) {
        let trace = &node.trace;

        if trace.address == CHEATCODE_ADDRESS || trace.address == HARDHAT_CONSOLE_ADDRESS {
            return;
        }

        let is_create = trace.kind.is_any_create();

        // Only include top-level calls which accout for calldata and base (21.000)
        // cost. Only include Calls and Creates as only these calls are isolated
        // in inspector.
        if trace.depth > 1 && (trace.kind == CallKind::Call || is_create) {
            return;
        }

        // For proxy calls, we only want to report the outermost call to avoid
        // duplicating gas reports along the proxy chain. The inner calls will be
        // included as part of the proxy chain in the gas report of the implementation.
        if trace.kind == CallKind::DelegateCall
            && !is_create
            && let Some(parent) = node
                .parent
                .and_then(|parent_id| arena.nodes().get(parent_id))
            && is_proxy_selector_and_output(&parent.trace, trace)
        {
            return;
        }

        let proxy_chain = detect_proxy_chain(arena, node.idx);

        // Hide `node` to ensure no one accidentally uses it without considering the
        // special handling for proxy chains below.
        #[allow(unused_variables)]
        let node = ();

        // We always want to use the gas used from the original trace, even for proxy
        // calls, as it represents the actual call made by the user and includes the
        // full gas cost of the proxied call.
        let gas_used = trace.gas_used;

        // If the call is through a proxy, use the implementation's `TraceCall` for
        // decoding and reporting gas, as it reflects the actual code being executed.
        let trace = proxy_chain
            .as_ref()
            .and_then(|chain| chain.first().map(Deref::deref))
            .unwrap_or(trace);

        let Some(name) = decoder.contracts.get(&trace.address) else {
            return;
        };
        let contract_name = name.rsplit(':').next().unwrap_or(name);

        let decoded = || decoder.decode_function(trace);

        let status = if trace.is_revert() {
            GasReportExecutionStatus::Revert
        } else if trace.is_error() {
            GasReportExecutionStatus::Halt
        } else {
            GasReportExecutionStatus::Success
        };

        let contract_info = self.contracts.entry(name.clone()).or_default();
        if trace.kind.is_any_create() {
            trace!(contract_name, "adding create gas info");
            contract_info.deployments.push(DeploymentGasReport {
                gas: gas_used,
                size: trace.data.len() as u64,
                status,
            });
        } else if let Some(DecodedCallData { signature, .. }) = decoded().await.call_data {
            let name = signature.split('(').next().unwrap();
            // Contract deployment status is determined by the setUp function
            let is_setup = name.test_function_kind().is_setup();
            // Ignore any test functions
            let should_include = !name.test_function_kind().is_known();

            if is_setup {
                // The `setUp` can only happen for test contracts, which are only deployed once,
                // so we can safely retrieve the last deployment to override its
                // status based on the `setUp` function call.
                if let Some(last_deployment) = contract_info.deployments.last_mut() {
                    last_deployment.status = status;
                }
            } else if should_include {
                trace!(contract_name, signature, "adding gas info");
                let gas_info = contract_info
                    .functions
                    .entry(name.to_string())
                    .or_default()
                    .entry(signature.clone())
                    .or_default();
                gas_info.calls.push((gas_used, status));

                let proxy_names = proxy_chain.map_or(Vec::new(), |proxy_chain| {
                    proxy_chain
                        .into_iter()
                        .rev()
                        .map(|call_trace| {
                            decoder
                                .contracts
                                .get(&call_trace.address)
                                .cloned()
                                .unwrap_or_else(|| {
                                    trace!(
                                        address = ?call_trace.address,
                                        "unknown contract in proxy chain"
                                    );

                                    UNRECOGNIZED_CONTRACT_NAME.to_owned()
                                })
                        })
                        .collect()
                });

                gas_info.proxy_chains.push(proxy_names);
            }
        }
    }

    /// Finalizes the gas report by calculating the min, max, mean, and median
    /// for each function.
    #[must_use]
    pub fn finalize(mut self) -> Self {
        trace!("finalizing gas report");
        for contract in self.contracts.values_mut() {
            for sigs in contract.functions.values_mut() {
                for func in sigs.values_mut() {
                    let mut calls_gas = func.calls.iter().map(|(g, _)| *g).collect::<Vec<_>>();
                    calls_gas.sort_unstable();
                    func.min = calls_gas.first().copied().unwrap_or_default();
                    func.max = calls_gas.last().copied().unwrap_or_default();
                    func.mean = calc::mean(&calls_gas);
                    func.median = calc::median_sorted(&calls_gas);
                }
            }
        }
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct ContractInfo {
    pub deployments: Vec<DeploymentGasReport>,
    /// Function name -> Function signature -> `GasInfo`
    pub functions: BTreeMap<String, BTreeMap<String, GasInfo>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GasInfo {
    pub calls: Vec<(u64, GasReportExecutionStatus)>,
    /// Proxy chains per call, parallel to `calls`. Each entry is a list of
    /// contract names from outermost proxy to implementation, or empty if
    /// the call is not through a proxy.
    pub proxy_chains: Vec<Vec<String>>,
    pub min: u64,
    pub mean: u64,
    pub median: u64,
    pub max: u64,
}

impl From<GasReport> for edr_gas_report::GasReport {
    fn from(value: GasReport) -> Self {
        let contracts = value
            .contracts
            .into_iter()
            .map(|(contract_name, contract)| {
                let functions = contract
                    .functions
                    .into_iter()
                    .flat_map(|(_, sigs)| {
                        sigs.into_iter().map(|(sig, gas_info)| {
                            let reports = gas_info
                                .calls
                                .iter()
                                .zip(gas_info.proxy_chains)
                                .map(|((gas, status), proxy_chain)| FunctionGasReport {
                                    gas: *gas,
                                    status: status.clone(),
                                    proxy_chain,
                                })
                                .collect::<Vec<_>>();

                            (sig, reports)
                        })
                    })
                    .collect();

                (
                    contract_name,
                    ContractGasReport {
                        deployments: contract.deployments,
                        functions,
                    },
                )
            })
            .collect();

        Self { contracts }
    }
}
