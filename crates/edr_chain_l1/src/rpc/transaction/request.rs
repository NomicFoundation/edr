use edr_primitives::{Address, Bytes, B256, U256};
use edr_transaction::pooled::eip4844::Blob;

/// Represents _all_ transaction requests received from RPC
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", try_from = "L1RpcTransactionRequestWire")]
pub struct L1RpcTransactionRequest {
    /// from address
    pub from: Address,
    /// to address
    #[serde(default)]
    pub to: Option<Address>,
    /// legacy, gas Price
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub gas_price: Option<u128>,
    /// max base fee per gas sender is willing to pay
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub max_fee_per_gas: Option<u128>,
    /// miner tip
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub max_priority_fee_per_gas: Option<u128>,
    /// gas
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub gas: Option<u64>,
    /// value of th tx in wei
    pub value: Option<U256>,
    /// Any additional data sent, deserialized from `data` or `input`
    pub data: Option<Bytes>,
    /// Transaction nonce
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub nonce: Option<u64>,
    /// Chain ID
    #[serde(default, with = "alloy_serde::quantity::opt")]
    pub chain_id: Option<u64>,
    /// warm storage access pre-payment
    #[serde(default)]
    pub access_list: Option<Vec<edr_eip2930::AccessListItem>>,
    /// EIP-2718 type
    #[serde(default, rename = "type", with = "alloy_serde::quantity::opt")]
    pub transaction_type: Option<u8>,
    /// Blobs (EIP-4844)
    pub blobs: Option<Vec<Blob>>,
    /// Blob versioned hashes (EIP-4844)
    pub blob_hashes: Option<Vec<B256>>,
    /// Authorizations are used to temporarily set the code of its signer to
    /// the code referenced by `address`. These also include a `chain_id` (which
    /// can be set to zero and not evaluated) as well as an optional `nonce`.
    #[serde(default)]
    pub authorization_list: Option<Vec<edr_eip7702::SignedAuthorization>>,
}

/// Error when a transaction request sets `data` and `input` to different
/// values. The message matches geth.
#[derive(Debug, thiserror::Error)]
#[error("both \"data\" and \"input\" are set and not equal. Please use \"input\" to pass transaction call data")]
pub struct InputMismatchError;

/// The wire format of [`L1RpcTransactionRequest`], which accepts both `data`
/// and `input`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct L1RpcTransactionRequestWire {
    from: Address,
    #[serde(default)]
    to: Option<Address>,
    #[serde(default, with = "alloy_serde::quantity::opt")]
    gas_price: Option<u128>,
    #[serde(default, with = "alloy_serde::quantity::opt")]
    max_fee_per_gas: Option<u128>,
    #[serde(default, with = "alloy_serde::quantity::opt")]
    max_priority_fee_per_gas: Option<u128>,
    #[serde(default, with = "alloy_serde::quantity::opt")]
    gas: Option<u64>,
    value: Option<U256>,
    data: Option<Bytes>,
    input: Option<Bytes>,
    #[serde(default, with = "alloy_serde::quantity::opt")]
    nonce: Option<u64>,
    #[serde(default, with = "alloy_serde::quantity::opt")]
    chain_id: Option<u64>,
    #[serde(default)]
    access_list: Option<Vec<edr_eip2930::AccessListItem>>,
    #[serde(default, rename = "type", with = "alloy_serde::quantity::opt")]
    transaction_type: Option<u8>,
    blobs: Option<Vec<Blob>>,
    blob_hashes: Option<Vec<B256>>,
    #[serde(default)]
    authorization_list: Option<Vec<edr_eip7702::SignedAuthorization>>,
}

impl TryFrom<L1RpcTransactionRequestWire> for L1RpcTransactionRequest {
    type Error = InputMismatchError;

    fn try_from(value: L1RpcTransactionRequestWire) -> Result<Self, Self::Error> {
        Ok(Self {
            from: value.from,
            to: value.to,
            gas_price: value.gas_price,
            max_fee_per_gas: value.max_fee_per_gas,
            max_priority_fee_per_gas: value.max_priority_fee_per_gas,
            gas: value.gas,
            value: value.value,
            data: match (value.data, value.input) {
                (Some(data), Some(input)) if data != input => return Err(InputMismatchError),
                (data, input) => input.or(data),
            },
            nonce: value.nonce,
            chain_id: value.chain_id,
            access_list: value.access_list,
            transaction_type: value.transaction_type,
            blobs: value.blobs,
            blob_hashes: value.blob_hashes,
            authorization_list: value.authorization_list,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_and_input() -> anyhow::Result<()> {
        const FROM: &str = r#""from":"0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266""#;

        let parse = |fields: &str| {
            serde_json::from_str::<L1RpcTransactionRequest>(&format!("{{{FROM},{fields}}}"))
        };

        let with_data = parse(r#""data":"0x8b1329e0""#)?;
        let with_input = parse(r#""input":"0x8b1329e0""#)?;
        let with_both = parse(r#""data":"0x8b1329e0","input":"0x8b1329e0""#)?;
        assert_eq!(
            with_data.data,
            Some(Bytes::from_static(&[0x8b, 0x13, 0x29, 0xe0]))
        );
        assert_eq!(with_input, with_data);
        assert_eq!(with_both, with_data);

        let error = parse(r#""data":"0x8b1329e0","input":"0x12345678""#)
            .expect_err("Should fail due to different data and input");
        assert_eq!(error.to_string(), InputMismatchError.to_string());

        // An empty `input` differs from non-empty `data`
        let error = parse(r#""data":"0x1234","input":"0x""#)
            .expect_err("Should fail due to different data and input");
        assert_eq!(error.to_string(), InputMismatchError.to_string());

        // Equal bytes in different hex case are the same calldata
        let with_mixed_case = parse(r#""data":"0xAB","input":"0xab""#)?;
        assert_eq!(with_mixed_case.data, Some(Bytes::from_static(&[0xab])));

        Ok(())
    }
}
