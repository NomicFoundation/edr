use std::str::FromStr as _;

use edr_primitives::Address;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct RpcAddress(pub Address);

impl From<Address> for RpcAddress {
    fn from(address: Address) -> Self {
        Self(address)
    }
}

impl From<RpcAddress> for Address {
    fn from(rpc_address: RpcAddress) -> Self {
        rpc_address.0
    }
}

impl<'deserializer> serde::Deserialize<'deserializer> for RpcAddress {
    fn deserialize<DeserializerT>(deserializer: DeserializerT) -> Result<Self, DeserializerT::Error>
    where
        DeserializerT: serde::Deserializer<'deserializer>,
    {
        let value = String::deserialize(deserializer).map_err(|error| {
            if let Some(value) = extract_value_from_serde_json_error(error.to_string().as_str()) {
                serde::de::Error::custom(format!(
                    "This method only supports strings but input was: {value}"
                ))
            } else {
                serde::de::Error::custom(format!(
                    "Failed to deserialize address argument into string with error: '{error}'"
                ))
            }
        })?;

        let error_message =
            || serde::de::Error::custom(format!("invalid value \"{value}\" supplied to : ADDRESS"));

        if !value.starts_with("0x") {
            return Err(error_message());
        }

        if value.len() != 42 {
            return Err(error_message());
        }

        Address::from_str(&value).map_or_else(
            |_error| Err(error_message()),
            |address| Ok(RpcAddress(address)),
        )
    }
}

fn extract_value_from_serde_json_error(error_message: &str) -> Option<&str> {
    error_message
        .split_once('`')
        .and_then(|(_, rest)| rest.rsplit_once('`').map(|(value, _)| value))
}
