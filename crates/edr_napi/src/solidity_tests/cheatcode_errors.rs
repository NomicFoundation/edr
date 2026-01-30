use napi_derive::napi;
use serde::Serialize;

#[napi(string_enum)]
#[derive(Serialize)]
#[doc = "Error codes that can be returned by cheatcodes in Solidity tests."]
pub enum CheatcodeErrorCode {
    #[doc = "The specified cheatcode is not supported."]
    UnsupportedCheatcode,
    #[doc = "The specified cheatcode is missing"]
    MissingCheatcode,
}

#[napi(object)]
#[derive(Clone, Serialize)]
#[doc = "Represents an error returned by a cheatcode in Solidity tests."]
pub struct CheatcodeErrorDetails {
    #[doc = "The error code representing the type of cheatcode error."]
    pub code: CheatcodeErrorCode,
    #[doc = "The name of the cheatcode that caused the error."]
    pub cheatcode: String,
}
