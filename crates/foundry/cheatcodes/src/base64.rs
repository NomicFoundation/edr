use alloy_sol_types::SolValue;
use base64::prelude::*;

use crate::{
    impl_is_pure_true, Cheatcode, Cheatcodes, Result,
    Vm::{toBase64URL_0Call, toBase64URL_1Call, toBase64_0Call, toBase64_1Call},
};

impl_is_pure_true!(toBase64_0Call);
impl Cheatcode for toBase64_0Call {
    fn apply(&self, _state: &mut Cheatcodes) -> Result {
        let Self { data } = self;
        Ok(BASE64_STANDARD.encode(data).abi_encode())
    }
}

impl_is_pure_true!(toBase64_1Call);
impl Cheatcode for toBase64_1Call {
    fn apply(&self, _state: &mut Cheatcodes) -> Result {
        let Self { data } = self;
        Ok(BASE64_STANDARD.encode(data).abi_encode())
    }
}

impl_is_pure_true!(toBase64URL_0Call);
impl Cheatcode for toBase64URL_0Call {
    fn apply(&self, _state: &mut Cheatcodes) -> Result {
        let Self { data } = self;
        Ok(BASE64_URL_SAFE.encode(data).abi_encode())
    }
}

impl_is_pure_true!(toBase64URL_1Call);
impl Cheatcode for toBase64URL_1Call {
    fn apply(&self, _state: &mut Cheatcodes) -> Result {
        let Self { data } = self;
        Ok(BASE64_URL_SAFE.encode(data).abi_encode())
    }
}
