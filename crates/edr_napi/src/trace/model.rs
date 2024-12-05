use napi_derive::napi;
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[allow(non_camel_case_types)] // intentionally mimicks the original case in TS
#[allow(clippy::upper_case_acronyms)]
#[napi]
// Mimicks [`edr_solidity::build_model::ContractFunctionType`].
pub enum ContractFunctionType {
    CONSTRUCTOR,
    FUNCTION,
    FALLBACK,
    RECEIVE,
    GETTER,
    MODIFIER,
    FREE_FUNCTION,
}

impl From<edr_solidity::build_model::ContractFunctionType> for ContractFunctionType {
    fn from(value: edr_solidity::build_model::ContractFunctionType) -> Self {
        match value {
            edr_solidity::build_model::ContractFunctionType::Constructor => Self::CONSTRUCTOR,
            edr_solidity::build_model::ContractFunctionType::Function => Self::FUNCTION,
            edr_solidity::build_model::ContractFunctionType::Fallback => Self::FALLBACK,
            edr_solidity::build_model::ContractFunctionType::Receive => Self::RECEIVE,
            edr_solidity::build_model::ContractFunctionType::Getter => Self::GETTER,
            edr_solidity::build_model::ContractFunctionType::Modifier => Self::MODIFIER,
            edr_solidity::build_model::ContractFunctionType::FreeFunction => Self::FREE_FUNCTION,
        }
    }
}
