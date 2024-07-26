//! Convenience functions.

use alloy_json_abi as abi;

/// Converts a JSON ABI error item to its selector.
pub fn json_abi_error_selector(error_abi_item: &serde_json::Value) -> Result<[u8; 4], Box<str>> {
    // Unfortunately, alloy_json_abi does not allow deserializing from owned values
    let value = error_abi_item.to_string();
    Ok(*serde_json::from_str::<abi::Error>(&value)
        .map_err(|e| e.to_string().into_boxed_str())?
        .selector())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_error_selector() {
        let value = json!({
          "type": "error",
          "name": "MyCustomError",
          "inputs": [
            { "type": "account", "name": "owner" },
            { "type": "uint256", "name": "balance"}
          ]

        });

        assert_eq!(
            json_abi_error_selector(&value),
            Ok([0xec, 0xcc, 0x91, 0xab])
        );

        let value = json!({ "type": "error", "name": "Unauthorized", "inputs": [] });

        assert_eq!(
            json_abi_error_selector(&value),
            Ok([0x82, 0xb4, 0x29, 0x00])
        );
    }
}
