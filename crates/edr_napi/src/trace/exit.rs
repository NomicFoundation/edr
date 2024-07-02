use napi::{
    bindgen_prelude::FromNapiValue,
    sys::{napi_env__, napi_value__},
};
use napi_derive::napi;

#[napi]
pub struct Exit(pub(crate) edr_solidity::exit::ExitCode);

impl FromNapiValue for Exit {
    unsafe fn from_napi_value(
        env: *mut napi_env__,
        napi_val: *mut napi_value__,
    ) -> napi::Result<Self> {
        let value = u8::from_napi_value(env, napi_val)?;

        let code = edr_solidity::exit::ExitCode::try_from(value)
            .map_err(|_err| napi::Error::from_reason("Invalid exit code"))?;

        Ok(Exit(code))
    }
}

#[napi]
impl Exit {
    #[napi(getter)]
    pub fn kind(&self) -> u8 {
        self.0 as u8
    }

    #[napi]
    pub fn is_error(&self) -> bool {
        self.0.is_error()
    }

    #[napi]
    pub fn get_reason(&self) -> String {
        self.0.to_string()
    }
}
