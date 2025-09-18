use std::collections::HashMap;

use napi::bindgen_prelude::BigInt;
use napi_derive::napi;

#[napi(object)]
pub struct GasReport {
    pub contracts: HashMap<String, ContractGasReport>,
}

#[napi(object)]
pub struct ContractGasReport {
    pub deployments: Vec<DeploymentGasReport>,
    pub functions: HashMap<String, Vec<FunctionGasReport>>,
}

#[napi]
pub enum GasReportExecutionStatus {
    Success,
    Revert,
    Halt,
}

#[napi(object)]
pub struct DeploymentGasReport {
    pub gas: BigInt,
    pub size: BigInt,
    pub status: GasReportExecutionStatus,
}

#[napi(object)]
pub struct FunctionGasReport {
    pub gas: BigInt,
    pub status: GasReportExecutionStatus,
}

impl From<edr_provider::gas_reports::GasReport> for GasReport {
    fn from(value: edr_provider::gas_reports::GasReport) -> Self {
        Self {
            contracts: value
                .into_inner()
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        }
    }
}

impl From<edr_provider::gas_reports::ContractGasReport> for ContractGasReport {
    fn from(value: edr_provider::gas_reports::ContractGasReport) -> Self {
        Self {
            deployments: value.deployments.into_iter().map(Into::into).collect(),
            functions: value
                .functions
                .into_iter()
                .map(|(k, v)| {
                    let function_reports = v.into_iter().map(FunctionGasReport::from).collect();
                    (k, function_reports)
                })
                .collect(),
        }
    }
}

impl From<edr_provider::gas_reports::GasReportExecutionStatus> for GasReportExecutionStatus {
    fn from(value: edr_provider::gas_reports::GasReportExecutionStatus) -> Self {
        match value {
            edr_provider::gas_reports::GasReportExecutionStatus::Success => Self::Success,
            edr_provider::gas_reports::GasReportExecutionStatus::Revert => Self::Revert,
            edr_provider::gas_reports::GasReportExecutionStatus::Halt => Self::Halt,
        }
    }
}

impl From<edr_provider::gas_reports::DeploymentGasReport> for DeploymentGasReport {
    fn from(value: edr_provider::gas_reports::DeploymentGasReport) -> Self {
        Self {
            gas: BigInt::from(value.gas),
            size: BigInt::from(value.size),
            status: value.status.into(),
        }
    }
}

impl From<edr_provider::gas_reports::FunctionGasReport> for FunctionGasReport {
    fn from(value: edr_provider::gas_reports::FunctionGasReport) -> Self {
        Self {
            gas: BigInt::from(value.gas),
            status: value.status.into(),
        }
    }
}
