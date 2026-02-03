//! Rewrite of `hardhat-network/provider/return-data.ts` from Hardhat.

use alloy_sol_types::SolError;
use edr_primitives::{Bytes, U256};

// Built-in error types
// See <https://docs.soliditylang.org/en/v0.8.26/control-structures.html#error-handling-assert-require-revert-and-exceptions>
alloy_sol_types::sol! {
  error Error(string);
  error Panic(uint256);
  error CheatcodeError(string);

  #[derive(Debug)]
  enum CheatcodeErrorCode {
    /// The cheatcode is not supported.
    UnsupportedCheatcode,
    /// The cheatcode is missing.
    MissingCheatcode,
  }

  #[derive(Debug)]
  struct CheatcodeErrorDetails {
    CheatcodeErrorCode code;
    string cheatcode;
  }

  error StructuredCheatcodeError(CheatcodeErrorDetails err);
}

/// A wrapper around return data from a Solidity function call.
pub struct ReturnData<'a> {
    /// The raw return data.
    pub value: &'a Bytes,
    selector: Option<[u8; 4]>,
}

impl<'a> ReturnData<'a> {
    /// Creates a new `ReturnData` instance from the given bytes.
    pub fn new(value: &'a Bytes) -> Self {
        let selector = value
            .get(0..4)
            .map(|selector| selector.try_into().expect("selector is 4 bytes"));

        Self { value, selector }
    }

    /// Returns true if the return data is empty.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Returns true if the return data matches the given selector.
    pub fn matches_selector(&self, selector: impl AsRef<[u8]>) -> bool {
        self.selector
            .is_some_and(|value| value == selector.as_ref())
    }

    /// Returns true if the return data represents a Solidity `Error(string)`
    /// revert.
    pub fn is_error_return_data(&self) -> bool {
        self.selector == Some(Error::SELECTOR)
    }

    /// Returns true if the return data represents a Solidity test cheatcode
    /// `CheatcodeError(string)` revert.
    pub fn is_cheatcode_error_return_data(&self) -> bool {
        self.selector == Some(CheatcodeError::SELECTOR)
    }

    /// Returns true if the return data represents a Solidity test cheatcode
    /// `StructuredCheatcodeError(CheatcodeErrorDetails)` revert.
    pub fn is_structured_cheatcode_error_return_data(&self) -> bool {
        self.selector == Some(StructuredCheatcodeError::SELECTOR)
    }

    /// Returns true if the return data represents a Solidity `Panic(uint256)`
    /// revert.
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

    /// Decodes the structured cheatcode error details from the return data.
    pub fn decode_structured_cheatcode_error(
        &self,
    ) -> Result<CheatcodeErrorDetails, alloy_sol_types::Error> {
        StructuredCheatcodeError::abi_decode(&self.value[..]).map(|e| e.err)
    }
}
