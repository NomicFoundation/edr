use edr_jsonrpc_error_macro::rpc_error;

// Unknown serde keys are denied by default — the whitelist is closed.
#[rpc_error(tag = "bad")]
#[serde(some_future_serde_attribute)]
pub struct Bad {
    x: String,
}

fn main() {}
