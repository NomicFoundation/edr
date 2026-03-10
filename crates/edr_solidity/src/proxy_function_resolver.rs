//! Proxy contract function selector resolution.
//!
//! This module provides functionality to resolve function selectors for proxy
//! contracts, including:
//! - ERC-1967 implementation slot detection
//! - Fallback search across all known contracts
//!
//! See [ERC-1967](https://eips.ethereum.org/EIPS/eip-1967) for more details
//! on the standard proxy storage slots.

use std::collections::HashSet;

use edr_primitives::{Address, U256};

/// ERC-1967 implementation storage slot.
/// Derived from: `keccak256("eip1967.proxy.implementation") - 1`
pub const ERC1967_IMPL_SLOT: U256 = U256::from_be_bytes([
    0x36, 0x08, 0x94, 0xa1, 0x3b, 0xa1, 0xa3, 0x21, 0x06, 0x67, 0xc8, 0x28, 0x49, 0x2d, 0xb9, 0x8d,
    0xca, 0x3e, 0x20, 0x76, 0xcc, 0x37, 0x35, 0xa9, 0x20, 0xa3, 0xca, 0x50, 0x5d, 0x38, 0x2b, 0xbc,
]);

/// ERC-1967 beacon storage slot.
/// Derived from: `keccak256("eip1967.proxy.beacon") - 1`
pub const ERC1967_BEACON_SLOT: U256 = U256::from_be_bytes([
    0xa3, 0xf0, 0xad, 0x74, 0xe5, 0x42, 0x3a, 0xeb, 0xfd, 0x80, 0xd3, 0xef, 0x43, 0x46, 0x57, 0x83,
    0x35, 0xa9, 0xa7, 0x2a, 0xea, 0xee, 0x59, 0xff, 0x6c, 0xb3, 0x58, 0x2b, 0x35, 0x13, 0x3d, 0x50,
]);

/// ERC-1967 admin storage slot.
/// Derived from: `keccak256("eip1967.proxy.admin") - 1`
#[allow(dead_code)]
pub const ERC1967_ADMIN_SLOT: U256 = U256::from_be_bytes([
    0xb5, 0x31, 0x27, 0x68, 0x4a, 0x56, 0x8b, 0x31, 0x73, 0xae, 0x13, 0xb9, 0xf8, 0xa6, 0x01, 0x6e,
    0x24, 0x3e, 0x63, 0xb6, 0xe8, 0xee, 0x11, 0x78, 0xd6, 0xa7, 0x17, 0x85, 0x0b, 0x5d, 0x61, 0x03,
]);

/// How a function selector was resolved for a proxy contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedFrom {
    /// Resolved via ERC-1967 implementation slot.
    Implementation {
        /// The name of the implementation contract.
        contract_name: String,
        /// The address of the implementation contract.
        address: Address,
    },
    /// Resolved via ERC-1967 beacon slot.
    Beacon {
        /// The address of the beacon contract.
        beacon_address: Address,
        /// The address of the implementation contract.
        impl_address: Address,
        /// The name of the implementation contract.
        contract_name: String,
    },
    /// Resolved via fallback search across all known contracts.
    /// No additional info is included in the output for this case.
    Fallback,
}

/// A decoded function with information about how it was resolved.
#[derive(Debug, Clone)]
pub struct DecodedFunction {
    /// The function signature (e.g., "transfer(address,uint256)").
    pub signature: String,
    /// How the function was resolved, if not directly from the called
    /// contract.
    pub resolved_from: Option<ResolvedFrom>,
}

impl DecodedFunction {
    /// Creates a new decoded function that was resolved directly.
    pub fn direct(signature: String) -> Self {
        Self {
            signature,
            resolved_from: None,
        }
    }

    /// Creates a new decoded function resolved via ERC-1967 implementation
    /// slot.
    pub fn from_implementation(
        signature: String,
        contract_name: String,
        address: Address,
    ) -> Self {
        Self {
            signature,
            resolved_from: Some(ResolvedFrom::Implementation {
                contract_name,
                address,
            }),
        }
    }

    /// Creates a new decoded function resolved via fallback search.
    pub fn from_fallback(signature: String) -> Self {
        Self {
            signature,
            resolved_from: Some(ResolvedFrom::Fallback),
        }
    }

    /// Creates an unrecognized function result.
    pub fn unrecognized() -> Self {
        Self {
            signature: super::solidity_stack_trace::UNRECOGNIZED_FUNCTION_NAME.to_string(),
            resolved_from: None,
        }
    }
}

/// Trait for reading state during proxy function resolution.
pub trait StateReader {
    /// Read a storage slot from the given address.
    fn storage(&self, address: Address, index: U256) -> Option<U256>;

    /// Get the code at the given address.
    fn code(&self, address: Address) -> Option<Vec<u8>>;
}

/// A proxy function resolver that can resolve function selectors for proxy
/// contracts.
pub struct ProxyFunctionResolver<'a, S: StateReader> {
    state: Option<&'a S>,
}

impl<'a, S: StateReader> ProxyFunctionResolver<'a, S> {
    /// Creates a new resolver with the given state reader.
    pub fn with_state(state: &'a S) -> Self {
        Self { state: Some(state) }
    }

    /// Creates a new resolver without state access (fallback only).
    pub fn without_state() -> Self {
        Self { state: None }
    }

    /// Try to resolve the implementation address from ERC-1967 implementation
    /// slot.
    pub fn get_erc1967_implementation(&self, proxy_address: Address) -> Option<Address> {
        let state = self.state?;
        let slot_value = state.storage(proxy_address, ERC1967_IMPL_SLOT)?;

        // Check if the slot is non-zero
        if slot_value == U256::ZERO {
            return None;
        }

        // Extract address from the last 20 bytes of the 32-byte slot value
        let bytes = slot_value.to_be_bytes::<32>();
        Some(Address::from_slice(&bytes[12..32]))
    }

    /// Try to resolve the beacon address from ERC-1967 beacon slot.
    #[allow(dead_code)]
    pub fn get_erc1967_beacon(&self, proxy_address: Address) -> Option<Address> {
        let state = self.state?;
        let slot_value = state.storage(proxy_address, ERC1967_BEACON_SLOT)?;

        // Check if the slot is non-zero
        if slot_value == U256::ZERO {
            return None;
        }

        // Extract address from the last 20 bytes of the 32-byte slot value
        let bytes = slot_value.to_be_bytes::<32>();
        Some(Address::from_slice(&bytes[12..32]))
    }
}

/// Result of searching for a function selector across all known contracts.
#[derive(Debug, Clone)]
pub struct SelectorSearchResult {
    /// The matching function signatures. Usually there's only one, but
    /// selector collisions are theoretically possible.
    pub signatures: HashSet<String>,
}

impl SelectorSearchResult {
    /// Returns the signature if there's exactly one match.
    pub fn single_signature(&self) -> Option<&String> {
        if self.signatures.len() == 1 {
            self.signatures.iter().next()
        } else {
            None
        }
    }

    /// Formats the result as a string. If there are multiple matches
    /// (selector collision), they are joined with " or ".
    pub fn format_signature(&self) -> Option<String> {
        if self.signatures.is_empty() {
            return None;
        }

        if self.signatures.len() == 1 {
            return self.signatures.iter().next().cloned();
        }

        // Multiple matches - join with " or "
        let mut signatures: Vec<_> = self.signatures.iter().collect();
        signatures.sort(); // Consistent ordering
        Some(signatures.into_iter().cloned().collect::<Vec<_>>().join(" or "))
    }
}

/// Formats a contract call for console output, including proxy resolution
/// information.
pub fn format_contract_call(
    contract_name: &str,
    decoded: &DecodedFunction,
) -> String {
    match &decoded.resolved_from {
        // Direct match in called contract or fallback resolution
        None | Some(ResolvedFrom::Fallback) => {
            format!("{}#{}", contract_name, decoded.signature)
        }
        // Resolved via ERC-1967 implementation slot
        Some(ResolvedFrom::Implementation {
            contract_name: impl_name,
            address,
        }) => {
            format!(
                "{}#{} (impl: {} @ {:#x})",
                contract_name, decoded.signature, impl_name, address
            )
        }
        // Resolved via ERC-1967 beacon
        Some(ResolvedFrom::Beacon {
            impl_address,
            contract_name: impl_name,
            ..
        }) => {
            format!(
                "{}#{} (beacon impl: {} @ {:#x})",
                contract_name, decoded.signature, impl_name, impl_address
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_erc1967_impl_slot_value() {
        // Verify the slot value matches the expected keccak256 - 1
        // keccak256("eip1967.proxy.implementation") - 1 =
        // 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc
        let expected = U256::from_str_radix(
            "360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc",
            16,
        )
        .unwrap();
        assert_eq!(ERC1967_IMPL_SLOT, expected);
    }

    #[test]
    fn test_erc1967_beacon_slot_value() {
        // Verify the slot value matches the expected keccak256 - 1
        // keccak256("eip1967.proxy.beacon") - 1 =
        // 0xa3f0ad74e5423aebfd80d3ef4346578335a9a72aeaee59ff6cb3582b35133d50
        let expected = U256::from_str_radix(
            "a3f0ad74e5423aebfd80d3ef4346578335a9a72aeaee59ff6cb3582b35133d50",
            16,
        )
        .unwrap();
        assert_eq!(ERC1967_BEACON_SLOT, expected);
    }

    #[test]
    fn test_decoded_function_direct() {
        let decoded = DecodedFunction::direct("transfer(address,uint256)".to_string());
        assert_eq!(decoded.signature, "transfer(address,uint256)");
        assert!(decoded.resolved_from.is_none());
    }

    #[test]
    fn test_decoded_function_from_implementation() {
        let decoded = DecodedFunction::from_implementation(
            "transfer(address,uint256)".to_string(),
            "MyToken".to_string(),
            Address::repeat_byte(0x42),
        );
        assert_eq!(decoded.signature, "transfer(address,uint256)");
        assert!(matches!(
            decoded.resolved_from,
            Some(ResolvedFrom::Implementation { .. })
        ));
    }

    #[test]
    fn test_decoded_function_from_fallback() {
        let decoded = DecodedFunction::from_fallback("transfer(address,uint256)".to_string());
        assert_eq!(decoded.signature, "transfer(address,uint256)");
        assert!(matches!(
            decoded.resolved_from,
            Some(ResolvedFrom::Fallback)
        ));
    }

    #[test]
    fn test_format_contract_call_direct() {
        let decoded = DecodedFunction::direct("transfer(address,uint256)".to_string());
        let result = format_contract_call("MyProxy", &decoded);
        assert_eq!(result, "MyProxy#transfer(address,uint256)");
    }

    #[test]
    fn test_format_contract_call_fallback() {
        let decoded = DecodedFunction::from_fallback("transfer(address,uint256)".to_string());
        let result = format_contract_call("MyProxy", &decoded);
        assert_eq!(result, "MyProxy#transfer(address,uint256)");
    }

    #[test]
    fn test_format_contract_call_implementation() {
        let impl_addr = Address::repeat_byte(0xab);
        let decoded = DecodedFunction::from_implementation(
            "transfer(address,uint256)".to_string(),
            "MyToken".to_string(),
            impl_addr,
        );
        let result = format_contract_call("EIP173Proxy", &decoded);
        assert!(result.contains("EIP173Proxy#transfer(address,uint256)"));
        assert!(result.contains("impl: MyToken"));
        assert!(result.contains("0xab"));
    }

    #[test]
    fn test_selector_search_result_single() {
        let mut result = SelectorSearchResult {
            signatures: HashSet::new(),
        };
        result.signatures.insert("transfer(address,uint256)".to_string());

        assert_eq!(
            result.single_signature(),
            Some(&"transfer(address,uint256)".to_string())
        );
        assert_eq!(
            result.format_signature(),
            Some("transfer(address,uint256)".to_string())
        );
    }

    #[test]
    fn test_selector_search_result_multiple() {
        let mut result = SelectorSearchResult {
            signatures: HashSet::new(),
        };
        result.signatures.insert("funcA()".to_string());
        result.signatures.insert("funcB()".to_string());

        assert!(result.single_signature().is_none());
        let formatted = result.format_signature().unwrap();
        assert!(formatted.contains(" or "));
        assert!(formatted.contains("funcA()"));
        assert!(formatted.contains("funcB()"));
    }

    #[test]
    fn test_selector_search_result_empty() {
        let result = SelectorSearchResult {
            signatures: HashSet::new(),
        };

        assert!(result.single_signature().is_none());
        assert!(result.format_signature().is_none());
    }
}
