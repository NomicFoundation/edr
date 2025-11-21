use alloy_eips::eip7840::BlobParams;
use alloy_hardforks::{mainnet, holesky, hoodi, sepolia};

/// EIP 7982 new node configuration for stablishing Blob Parameter only harforks
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ScheduledBlobParams {
    schedule: Vec<(u64, BlobParams)>
}

impl ScheduledBlobParams {
    pub fn mainnet() -> Self {
        vec![
            (mainnet::MAINNET_BPO1_TIMESTAMP, BlobParams::bpo1()),
            (mainnet::MAINNET_BPO2_TIMESTAMP, BlobParams::bpo2()),
        ].into()
    }
    pub fn holesky() -> Self {
        vec![
            (holesky::HOLESKY_BPO1_TIMESTAMP, BlobParams::bpo1()),
            (holesky::HOLESKY_BPO2_TIMESTAMP, BlobParams::bpo2()),
        ].into()
    }
    pub fn sepolia() -> Self {
        vec![
            (sepolia::SEPOLIA_BPO1_TIMESTAMP, BlobParams::bpo1()),
            (sepolia::SEPOLIA_BPO2_TIMESTAMP, BlobParams::bpo2()),
        ].into()
    }
    pub fn hoodi() -> Self {
        vec![
            (hoodi::HOODI_BPO1_TIMESTAMP, BlobParams::bpo1()),
            (hoodi::HOODI_BPO2_TIMESTAMP, BlobParams::bpo2()),
        ].into()
    }

    pub fn active_scheduled_params_at_timestamp(&self, timestamp: u64) -> Option<&BlobParams> {
        self.schedule.iter().rev().find(|(ts, _)| timestamp >= *ts).map(|(_, params)| params)
    }
}

impl From<Vec<(u64, BlobParams)>> for ScheduledBlobParams {
    fn from(value: Vec<(u64, BlobParams)>) -> Self {
        ScheduledBlobParams{
            schedule: value
        }
    }
}
