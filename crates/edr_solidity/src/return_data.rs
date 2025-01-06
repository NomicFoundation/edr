//! Rewrite of `hardhat-network/provider/return-data.ts` from Hardhat.

use alloy_sol_types::SolError;
use edr_eth::{Bytes, U256};

// Built-in error types
// See <https://docs.soliditylang.org/en/v0.8.26/control-structures.html#error-handling-assert-require-revert-and-exceptions>
alloy_sol_types::sol! {
  error Error(string);
  error Panic(uint256);
}

pub struct ReturnData {
    pub value: Bytes,
    selector: Option<[u8; 4]>,
}

impl ReturnData {
    pub fn new(value: Bytes) -> Self {
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
            .map_or(false, |value| value == selector.as_ref())
    }

    pub fn is_error_return_data(&self) -> bool {
        self.selector == Some(Error::SELECTOR)
    }

    pub fn is_panic_return_data(&self) -> bool {
        self.selector == Some(Panic::SELECTOR)
    }

    /// Decodes the panic error code from the return data.
    pub fn decode_panic(&self) -> Result<U256, alloy_sol_types::Error> {
        Panic::abi_decode(&self.value[..], false).map(|p| p._0)
    }
}
