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
    pub functions: HashMap<String, FunctionGasReport>,
}

#[napi]
pub enum GasReportFunctionStatus {
    Success,
    Revert,
    Halt,
}

#[napi(object)]
pub struct DeploymentGasReport {
    pub gas: BigInt,
    pub size: BigInt,
    pub status: GasReportFunctionStatus,
}

#[napi(object)]
pub struct FunctionGasReport {
    pub calls: Vec<FunctionCallGasReport>,
}

#[napi(object)]
pub struct FunctionCallGasReport {
    pub gas: BigInt,
    pub status: GasReportFunctionStatus,
}

impl From<edr_provider::gas_reports::GasReport> for GasReport {
    fn from(value: edr_provider::gas_reports::GasReport) -> Self {
        Self {
            contracts: value
                .contracts
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
                .map(|(k, v)| (k, v.into()))
                .collect(),
        }
    }
}

impl From<edr_provider::gas_reports::GasReportFunctionStatus> for GasReportFunctionStatus {
    fn from(value: edr_provider::gas_reports::GasReportFunctionStatus) -> Self {
        match value {
            edr_provider::gas_reports::GasReportFunctionStatus::Success => Self::Success,
            edr_provider::gas_reports::GasReportFunctionStatus::Revert => Self::Revert,
            edr_provider::gas_reports::GasReportFunctionStatus::Halt => Self::Halt,
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
            calls: value.calls.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<edr_provider::gas_reports::FunctionCallGasReport> for FunctionCallGasReport {
    fn from(value: edr_provider::gas_reports::FunctionCallGasReport) -> Self {
        Self {
            gas: BigInt::from(value.gas),
            status: value.status.into(),
        }
    }
}
