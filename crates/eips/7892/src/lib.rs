#![warn(missing_docs)]

//! Types related to EIP-7892.

use alloy_eips::eip7840::BlobParams;
use alloy_hardforks::{holesky, hoodi, mainnet, sepolia};

/// EIP 7982 new node configuration for stablishing Blob Parameter only harforks
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ScheduledBlobParams {
    schedule: Vec<(u64, BlobParams)>,
}

impl ScheduledBlobParams {
    /// Ethereum Mainnet Blob Parameter Only hardforks schedules
    pub fn mainnet() -> Self {
        vec![
            (mainnet::MAINNET_BPO1_TIMESTAMP, BlobParams::bpo1()),
            (mainnet::MAINNET_BPO2_TIMESTAMP, BlobParams::bpo2()),
        ]
        .into()
    }

    /// Holesky Blob Parameter Only hardforks schedules
    pub fn holesky() -> Self {
        vec![
            (holesky::HOLESKY_BPO1_TIMESTAMP, BlobParams::bpo1()),
            (holesky::HOLESKY_BPO2_TIMESTAMP, BlobParams::bpo2()),
        ]
        .into()
    }
    /// Sepolia Blob Parameter Only hardforks schedules
    pub fn sepolia() -> Self {
        vec![
            (sepolia::SEPOLIA_BPO1_TIMESTAMP, BlobParams::bpo1()),
            (sepolia::SEPOLIA_BPO2_TIMESTAMP, BlobParams::bpo2()),
        ]
        .into()
    }
    /// Hoodi Blob Parameter Only hardforks schedules
    pub fn hoodi() -> Self {
        vec![
            (hoodi::HOODI_BPO1_TIMESTAMP, BlobParams::bpo1()),
            (hoodi::HOODI_BPO2_TIMESTAMP, BlobParams::bpo2()),
        ]
        .into()
    }

    /// Determines the active `BlobParams` for a given timestamp, based on BPO
    /// hardfork schedules
    pub fn active_scheduled_params_at_timestamp(&self, timestamp: u64) -> Option<&BlobParams> {
        self.schedule
            .iter()
            .rev()
            .find(|(ts, _)| timestamp >= *ts)
            .map(|(_, params)| params)
    }
}

impl From<Vec<(u64, BlobParams)>> for ScheduledBlobParams {
    fn from(mut value: Vec<(u64, BlobParams)>) -> Self {
        value.sort_by(|(timestamp_a, _params_a), (timestamp_b, _params_b)| {
            timestamp_a.cmp(timestamp_b)
        });
        ScheduledBlobParams { schedule: value }
    }
}
