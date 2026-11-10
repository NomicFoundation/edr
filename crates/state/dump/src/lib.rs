//! A portable dump of the accounts in a state.
//!
//! The JSON format is compatible with the `accounts` section of Anvil's state
//! dumps, so a dump can be loaded by `anvil_loadState` and an Anvil dump can be
//! decoded here. Fields this format doesn't use, such as Anvil's blocks and
//! transactions, are ignored when decoding.
#![warn(missing_docs)]

use std::{collections::BTreeMap, io::Read as _};

use edr_primitives::{Address, Bytes, B256, U256};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde::{Deserialize, Serialize, Serializer};

/// The version of the format written by [`StateDump::encode`].
pub const STATE_DUMP_VERSION: u64 = 1;

const GZIP_MAGIC_BYTES: [u8; 2] = [0x1f, 0x8b];

/// An account in a [`StateDump`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct DumpedAccount {
    /// The balance of the account.
    pub balance: U256,
    /// The code of the account. Empty if the account has no code.
    pub code: Bytes,
    /// The nonce of the account.
    pub nonce: u64,
    /// The non-zero storage slots of the account.
    #[serde(serialize_with = "serialize_storage")]
    pub storage: BTreeMap<U256, U256>,
}

/// A dump of the accounts in a state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateDump {
    /// The accounts, ordered by address.
    pub accounts: BTreeMap<Address, DumpedAccount>,
}

/// An error that occurs when decoding a [`StateDump`].
#[derive(Debug, thiserror::Error)]
pub enum StateDumpDecodeError {
    /// The gzip data could not be decompressed.
    #[error("Failed to decompress the state dump: {0}")]
    Gzip(std::io::Error),
    /// The data is not a valid state dump.
    #[error("Failed to decode the state dump: {0}")]
    Json(serde_json::Error),
    /// The dump was written by a newer version of the format.
    #[error(
        "The state dump has version {version}, but the highest supported version is {STATE_DUMP_VERSION}."
    )]
    UnsupportedVersion {
        /// The version of the dump.
        version: u64,
    },
}

impl StateDump {
    /// Decodes a dump from JSON, or from gzip-compressed JSON.
    pub fn decode(bytes: &[u8]) -> Result<Self, StateDumpDecodeError> {
        let json = if bytes.starts_with(&GZIP_MAGIC_BYTES) {
            let mut json = Vec::new();
            GzDecoder::new(bytes)
                .read_to_end(&mut json)
                .map_err(StateDumpDecodeError::Gzip)?;
            json
        } else {
            bytes.to_vec()
        };

        let format: Format = serde_json::from_slice(&json).map_err(StateDumpDecodeError::Json)?;
        if let Some(version) = format.version
            && version > STATE_DUMP_VERSION
        {
            return Err(StateDumpDecodeError::UnsupportedVersion { version });
        }

        Ok(Self {
            accounts: format.accounts,
        })
    }

    /// Encodes the dump as gzip-compressed JSON.
    pub fn encode(&self) -> Vec<u8> {
        let format = FormatRef {
            version: STATE_DUMP_VERSION,
            block: (),
            accounts: &self.accounts,
            best_block_number: (),
        };

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        serde_json::to_writer(&mut encoder, &format)
            .expect("serializing to an in-memory writer cannot fail");
        encoder
            .finish()
            .expect("compressing to an in-memory buffer cannot fail")
    }
}

/// The decoded file. `version` is absent in Anvil dumps.
#[derive(Deserialize)]
struct Format {
    #[serde(default)]
    version: Option<u64>,
    accounts: BTreeMap<Address, DumpedAccount>,
}

/// The encoded file. Anvil requires the `block` and `best_block_number` keys,
/// so they are written as `null`.
#[derive(Serialize)]
struct FormatRef<'accounts> {
    version: u64,
    block: (),
    accounts: &'accounts BTreeMap<Address, DumpedAccount>,
    best_block_number: (),
}

fn serialize_storage<SerializerT: Serializer>(
    storage: &BTreeMap<U256, U256>,
    serializer: SerializerT,
) -> Result<SerializerT::Ok, SerializerT::Error> {
    serializer.collect_map(
        storage
            .iter()
            .map(|(index, value)| (B256::from(*index), B256::from(*value))),
    )
}

#[cfg(test)]
mod tests {
    use edr_primitives::{address, bytes};

    use super::*;

    fn dump() -> StateDump {
        StateDump {
            accounts: BTreeMap::from([
                (
                    address!("0x00000000000000000000000000000000000000aa"),
                    DumpedAccount {
                        balance: U256::from(5),
                        code: bytes!("6001"),
                        nonce: 3,
                        storage: BTreeMap::from([(U256::from(1), U256::from(42))]),
                    },
                ),
                (
                    address!("0x00000000000000000000000000000000000000bb"),
                    DumpedAccount {
                        balance: U256::MAX,
                        ..DumpedAccount::default()
                    },
                ),
            ]),
        }
    }

    #[test]
    fn round_trip() {
        let dump = dump();
        assert_eq!(StateDump::decode(&dump.encode()).unwrap(), dump);
    }

    #[test]
    fn encoding_is_deterministic() {
        assert_eq!(dump().encode(), dump().encode());
    }

    #[test]
    fn encodes_anvil_compatible_json() {
        let mut json = Vec::new();
        GzDecoder::new(dump().encode().as_slice())
            .read_to_end(&mut json)
            .unwrap();

        let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(value["version"], STATE_DUMP_VERSION);
        assert!(value["block"].is_null());
        assert!(value["best_block_number"].is_null());

        let account = &value["accounts"]["0x00000000000000000000000000000000000000aa"];
        assert_eq!(account["nonce"], 3);
        assert_eq!(account["balance"], "0x5");
        assert_eq!(account["code"], "0x6001");
        assert_eq!(
            account["storage"]
                ["0x0000000000000000000000000000000000000000000000000000000000000001"],
            "0x000000000000000000000000000000000000000000000000000000000000002a"
        );
    }

    #[test]
    fn decodes_plain_json() {
        let json = r#"{"accounts":{"0x00000000000000000000000000000000000000aa":{"nonce":3,"balance":"0x5","code":"0x6001","storage":{"0x1":"0x2a"}}}}"#;

        let dump = StateDump::decode(json.as_bytes()).unwrap();
        let account = &dump.accounts[&address!("0x00000000000000000000000000000000000000aa")];
        assert_eq!(account.storage[&U256::from(1)], U256::from(42));
    }

    #[test]
    fn decodes_anvil_dump_and_ignores_its_other_fields() {
        let json = r#"{
            "block": {"number": "0x2", "timestamp": "0x66cdc82b"},
            "accounts": {
                "0x00000000000000000000000000000000000000aa": {
                    "nonce": 1,
                    "balance": "0x21e19e0c9bab2400000",
                    "code": "0x",
                    "storage": {}
                }
            },
            "best_block_number": 2,
            "blocks": [],
            "transactions": [],
            "historical_states": null
        }"#;

        let dump = StateDump::decode(json.as_bytes()).unwrap();
        assert_eq!(dump.accounts.len(), 1);
    }

    #[test]
    fn rejects_a_newer_version() {
        let json = r#"{"version":2,"accounts":{}}"#;

        let error = StateDump::decode(json.as_bytes()).unwrap_err();
        assert!(matches!(
            error,
            StateDumpDecodeError::UnsupportedVersion { version: 2 }
        ));
    }

    #[test]
    fn rejects_invalid_data() {
        assert!(matches!(
            StateDump::decode(b"not json"),
            Err(StateDumpDecodeError::Json(_))
        ));
        assert!(matches!(
            StateDump::decode(&[0x1f, 0x8b, 0x00]),
            Err(StateDumpDecodeError::Gzip(_))
        ));
    }
}
