use edr_jsonrpc_error_macro::rpc_error;

#[rpc_error(tag = "bad")]
#[serde(transparent)]
pub struct Bad {
    x: String,
}

fn main() {}
