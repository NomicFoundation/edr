//! Rewrite of `hardhat-network/provider/return-data.ts` from Hardhat.

use alloy_sol_types::SolError;
use edr_eth::U256;
use napi::bindgen_prelude::{BigInt, Uint8Array};
use napi_derive::napi;

// Built-in error types
// See <https://docs.soliditylang.org/en/v0.8.26/control-structures.html#error-handling-assert-require-revert-and-exceptions>
alloy_sol_types::sol! {
  error Error(string);
  error Panic(uint256);
}

#[napi]
pub struct ReturnData {
    #[napi(readonly)]
    pub value: Uint8Array,
    selector: Option<[u8; 4]>,
}

#[napi]
impl ReturnData {
    #[napi(constructor)]
    pub fn new(value: Uint8Array) -> Self {
        let selector = if value.len() >= 4 {
            Some(value[0..4].try_into().unwrap())
        } else {
            None
        };

        Self { value, selector }
    }

    #[napi]
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn matches_selector(&self, selector: impl AsRef<[u8]>) -> bool {
        self.selector
            .map_or(false, |value| value == selector.as_ref())
    }

    #[napi]
    pub fn is_error_return_data(&self) -> bool {
        self.selector == Some(Error::SELECTOR)
    }

    #[napi]
    pub fn is_panic_return_data(&self) -> bool {
        self.selector == Some(Panic::SELECTOR)
    }

    #[napi]
    pub fn decode_error(&self) -> napi::Result<String> {
        if self.is_empty() {
            return Ok(String::new());
        }

        let result = Error::abi_decode(&self.value[..], false).map_err(|_err| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "Expected return data to be a Error(string) and contain a valid string",
            )
        })?;

        Ok(result._0)
    }

    #[napi]
    pub fn decode_panic(&self) -> napi::Result<BigInt> {
        let result = Panic::abi_decode(&self.value[..], false).map_err(|_err| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "Expected return data to be a Error(string) and contain a valid string",
            )
        })?;

        Ok(BigInt {
            sign_bit: false,
            words: result._0.as_limbs().to_vec(),
        })
    }
}

fn panic_error_code_to_reason(error_code: &U256) -> Option<&'static str> {
    let code = u64::try_from(error_code).ok()?;

    match code {
        0x1 => Some("Assertion error"),
        0x11 => Some("Arithmetic operation overflowed outside of an unchecked block"),
        0x12 => Some("Division or modulo division by zero"),
        0x21 => {
            Some("Tried to convert a value into an enum, but the value was too big or negative")
        }
        0x22 => Some("Incorrectly encoded storage byte array"),
        0x31 => Some(".pop() was called on an empty array"),
        0x32 => Some("Array accessed at an out-of-bounds or negative index"),
        0x41 => Some("Too much memory was allocated, or an array was created that is too large"),
        0x51 => Some("Called a zero-initialized variable of internal function type"),
        _ => None,
    }
}

pub fn panic_error_code_to_message(error_code: BigInt) -> napi::Result<String> {
    // NOTE: N-API BigInt also has little-endian limbs
    let code = U256::checked_from_limbs_slice(&error_code.words).ok_or_else(|| {
        napi::Error::new(
            napi::Status::InvalidArg,
            "Expected panic error code to fit uint256",
        )
    })?;

    let reason = panic_error_code_to_reason(&code);

    Ok(if let Some(reason) = reason {
        format!("reverted with panic code 0x{code:x} ({reason})")
    } else {
        format!("reverted with unknown panic code 0x{code:x}")
    })
}
