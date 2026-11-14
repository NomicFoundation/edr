//! L1 Ethereum types for `eth_call` and `debug_traceCall`.
use edr_primitives::{Address, Bytes, B256, U256};
use edr_transaction::pooled::eip4844::Blob;

pub type Request = L1CallRequest;

/// For specifying input to methods requiring a transaction object, like
/// `eth_call` and `eth_estimateGas`
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", from = "L1CallRequestWire")]
pub struct L1CallRequest {
    /// the address from which the transaction should be sent
    pub from: Option<Address>,
    /// the address to which the transaction should be sent
    pub to: Option<Address>,
    /// gas
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub gas: Option<u64>,
    /// gas price
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub gas_price: Option<u128>,
    /// max base fee per gas sender is willing to pay
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub max_fee_per_gas: Option<u128>,
    /// miner tip
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub max_priority_fee_per_gas: Option<u128>,
    /// transaction value
    pub value: Option<U256>,
    /// transaction data, deserialized from `data` or `input`
    pub data: Option<Bytes>,
    /// warm storage access pre-payment
    pub access_list: Option<Vec<edr_eip2930::AccessListItem>>,
    /// EIP-2718 type
    #[serde(default, rename = "type", with = "alloy_serde::quantity::opt")]
    pub transaction_type: Option<u8>,
    /// Blobs (EIP-4844)
    pub blobs: Option<Vec<Blob>>,
    /// Blob versioned hashes (EIP-4844)
    pub blob_hashes: Option<Vec<B256>>,
    /// Authorization list (EIP-7702)
    pub authorization_list: Option<Vec<edr_eip7702::SignedAuthorization>>,
}

/// The wire format of [`L1CallRequest`], which accepts both `data` and
/// `input`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct L1CallRequestWire {
    from: Option<Address>,
    to: Option<Address>,
    #[serde(default, with = "alloy_serde::quantity::opt")]
    gas: Option<u64>,
    #[serde(default, with = "alloy_serde::quantity::opt")]
    gas_price: Option<u128>,
    #[serde(default, with = "alloy_serde::quantity::opt")]
    max_fee_per_gas: Option<u128>,
    #[serde(default, with = "alloy_serde::quantity::opt")]
    max_priority_fee_per_gas: Option<u128>,
    value: Option<U256>,
    data: Option<Bytes>,
    input: Option<Bytes>,
    access_list: Option<Vec<edr_eip2930::AccessListItem>>,
    #[serde(default, rename = "type", with = "alloy_serde::quantity::opt")]
    transaction_type: Option<u8>,
    blobs: Option<Vec<Blob>>,
    blob_hashes: Option<Vec<B256>>,
    authorization_list: Option<Vec<edr_eip7702::SignedAuthorization>>,
}

impl From<L1CallRequestWire> for L1CallRequest {
    fn from(value: L1CallRequestWire) -> Self {
        Self {
            from: value.from,
            to: value.to,
            gas: value.gas,
            gas_price: value.gas_price,
            max_fee_per_gas: value.max_fee_per_gas,
            max_priority_fee_per_gas: value.max_priority_fee_per_gas,
            value: value.value,
            // As in geth's `eth_call`, `input` takes precedence over `data`.
            data: value.input.or(value.data),
            access_list: value.access_list,
            transaction_type: value.transaction_type,
            blobs: value.blobs,
            blob_hashes: value.blob_hashes,
            authorization_list: value.authorization_list,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_alias() -> anyhow::Result<()> {
        const JSON_WITH_DATA: &str = r#"{
            "from":"0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
            "to":"0x5fbdb2315678afecb367f032d93f642f64180aa3",
            "data":"0x8b1329e0"
        }"#;

        const JSON_WITH_INPUT: &str = r#"{
            "from":"0x0000000000000000000000000000000000000000",
            "input":"0x8b1329e0",
            "to":"0x5fbdb2315678afecb367f032d93f642f64180aa3"
        }"#;

        const JSON_WITH_BOTH: &str = r#"{
            "from":"0x0000000000000000000000000000000000000000",
            "data":"0x8b1329e0",
            "input":"0x8b1329e0",
            "to":"0x5fbdb2315678afecb367f032d93f642f64180aa3"
        }"#;

        const JSON_WITH_DIFFERENT: &str = r#"{
            "from":"0x0000000000000000000000000000000000000000",
            "data":"0x8b1329e0",
            "input":"0x12345678",
            "to":"0x5fbdb2315678afecb367f032d93f642f64180aa3"
        }"#;

        let with_data: L1CallRequest = serde_json::from_str(JSON_WITH_DATA)?;
        let with_input: L1CallRequest = serde_json::from_str(JSON_WITH_INPUT)?;
        assert_eq!(with_data.data, with_input.data);

        let with_both: L1CallRequest = serde_json::from_str(JSON_WITH_BOTH)?;
        assert_eq!(with_both.data, with_data.data);

        let with_different: L1CallRequest = serde_json::from_str(JSON_WITH_DIFFERENT)?;
        assert_eq!(
            with_different.data,
            Some(Bytes::from_static(&[0x12, 0x34, 0x56, 0x78]))
        );

        // Like geth, an empty `input` still takes precedence over `data`
        let with_empty_input: L1CallRequest = serde_json::from_str(
            r#"{"to":"0x5fbdb2315678afecb367f032d93f642f64180aa3","data":"0x1234","input":"0x"}"#,
        )?;
        assert_eq!(with_empty_input.data, Some(Bytes::new()));

        // Equal bytes in different hex case are the same calldata
        let with_mixed_case: L1CallRequest = serde_json::from_str(
            r#"{"to":"0x5fbdb2315678afecb367f032d93f642f64180aa3","data":"0xAB","input":"0xab"}"#,
        )?;
        assert_eq!(with_mixed_case.data, Some(Bytes::from_static(&[0xab])));

        Ok(())
    }
}
