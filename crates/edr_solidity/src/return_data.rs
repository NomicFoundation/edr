//! Rewrite of `hardhat-network/provider/return-data.ts` from Hardhat.

use alloy_sol_types::SolError;
use edr_eth::{Bytes, U256};

// Built-in error types
// See <https://docs.soliditylang.org/en/v0.8.26/control-structures.html#error-handling-assert-require-revert-and-exceptions>
alloy_sol_types::sol! {
  error Error(string);
  error Panic(uint256);
  error CheatcodeError(string);
}

pub struct ReturnData<'a> {
    pub value: &'a Bytes,
    selector: Option<[u8; 4]>,
}

impl<'a> ReturnData<'a> {
    pub fn new(value: &'a Bytes) -> Self {
        let selector = if value.len() >= 4 {
            Some(value[0..4].try_into().expect("checked length"))
        } else {
            None
        };

        Self { value, selector }
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn matches_selector(&self, selector: impl AsRef<[u8]>) -> bool {
        self.selector
            .is_some_and(|value| value == selector.as_ref())
    }

    pub fn is_error_return_data(&self) -> bool {
        self.selector == Some(Error::SELECTOR)
    }

    pub fn is_cheatcode_error_return_data(&self) -> bool {
        self.selector == Some(CheatcodeError::SELECTOR)
    }

    pub fn is_panic_return_data(&self) -> bool {
        self.selector == Some(Panic::SELECTOR)
    }

    /// Decodes the panic error code from the return data.
    pub fn decode_panic(&self) -> Result<U256, alloy_sol_types::Error> {
        Panic::abi_decode(&self.value[..]).map(|p| p.0)
    }

    /// Decodes the cheatcode error message from the return data.
    pub fn decode_cheatcode_error(&self) -> Result<String, alloy_sol_types::Error> {
        CheatcodeError::abi_decode(&self.value[..]).map(|p| p.0)
    }
}
