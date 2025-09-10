use edr_eth::{filter::LogOutput, Bytes};

use crate::{Block, BlockOverrides, StateOverrideOptions, Transaction, TransactionRequest};
// TODO: check what attributes are needed

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SimulatePayload<TxRequest> {
    pub block_state_calls: Vec<SimBlock<TxRequest>>,
    pub trace_transfers: bool,
    pub validation: bool,
    pub return_full_transactions: bool,
}
// should we make transaction request generic?
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SimBlock<TxRequest> {
    pub block_overrides: Option<BlockOverrides>,
    pub state_overrides: Option<StateOverrideOptions>,
    pub calls: Vec<TxRequest>,
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SimResult {
    pub block: Block<Transaction>,
    pub calls: Vec<SimCallResult>,
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SimError {
    // write error codes
    pub code: i32,
    pub message: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SimCallResult {
    pub status: bool,
    pub return_data: Bytes,
    pub gas_used: u64,
    pub logs: Vec<LogOutput>,
    pub error: Option<SimError>,
}
