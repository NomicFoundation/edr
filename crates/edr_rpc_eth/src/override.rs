use edr_eth::{
    block::overrides::HeaderOverrides, withdrawal::Withdrawal, Address, Bytes, HashMap, B256, U256,
};

/// Type representing a set of overrides for storage information.
pub type StorageOverride = HashMap<B256, U256>;

/// Options for overriding account information.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountOverrideOptions {
    /// Account balance override.
    pub balance: Option<U256>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "alloy_serde::quantity::opt"
    )]
    /// Account nonce override.
    pub nonce: Option<u64>,
    /// Account code override.
    pub code: Option<Bytes>,
    /// Account storage override. Mutually exclusive with `storage_diff`.
    #[serde(rename = "state")]
    pub storage: Option<StorageOverride>,
    /// Account storage diff override. Mutually exclusive with `storage`.
    #[serde(rename = "stateDiff")]
    pub storage_diff: Option<StorageOverride>,
    /// Precompile address relocation.
    pub move_precompile_to_address: Option<Address>,
}

/// Type representing a full set of overrides for account information.
pub type StateOverrideOptions = HashMap<Address, AccountOverrideOptions>;

// maybe try to convert from this to HeaderOverrides with defaults where needed
#[derive(Clone, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockOverrides {
    pub number: Option<u64>,
    pub prev_randao: Option<U256>,
    pub time: Option<u64>,
    pub gas_limit: Option<u64>,
    pub fee_recipient: Option<Address>,
    pub base_fee_per_gas: Option<U256>,
    pub withdrawals: Option<Vec<Withdrawal>>,
    pub blob_base_fee: Option<u64>,
}

impl From<BlockOverrides> for HeaderOverrides {
    fn from(overrides: BlockOverrides) -> Self {
        HeaderOverrides {
            number: overrides.number,
            // mix_hash: overrides
            //     .prev_randao
            //     .map(|r| B256::from_slice(&r.to_be_bytes())),
            timestamp: overrides.time,
            gas_limit: overrides.gas_limit,
            beneficiary: overrides.fee_recipient,
            // base_fee: overrides.base_fee_per_gas.map(|b| b.as_u128()),
            // withdrawals_root: None, // TODO: compute from withdrawals
            // blob_gas: overrides.blob_base_fee.map(BlobGas::from),
            ..Default::default()
        }
    }
}
