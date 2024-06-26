use napi::bindgen_prelude::{Object, Uint8Array};
use napi_derive::napi;
use serde_json::Value;

#[napi(object)]
pub struct ContractFunction {
    #[napi(readonly)]
    pub name: String,
    /// TODO: Replace with `ContractFunctionType`
    #[napi(readonly, js_name = "type")]
    pub r#type: u8, // enum but can't use since ts enums are not structurally typed
    // location: Reference<SourceLocation>,
    /// TODO: Replace with `SourceLocation``
    #[napi(readonly, ts_type = "any")]
    pub location: Object,
    /// TODO: Replace with `Contract`
    #[napi(readonly, ts_type = "any")]
    pub contract: Object,
    /// TODO: Replace with `ContractFunctionVisibility`
    #[napi(readonly)]
    pub visibility: Option<u8>,
    #[napi(readonly)]
    pub is_payable: Option<bool>,
    /// Fixed up by `Contract.correctSelector`
    pub selector: Option<Uint8Array>,
    #[napi(readonly)]
    pub param_types: Option<Vec<Value>>,
}
