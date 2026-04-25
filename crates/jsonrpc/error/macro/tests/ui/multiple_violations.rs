use edr_jsonrpc_error_macro::rpc_error;

// All three violations should be reported in one compile pass.
#[rpc_error(tag = "bad")]
#[serde(transparent)]
pub struct Bad {
    #[serde(flatten)]
    a: String,
    #[serde(some_garbage)]
    b: u32,
}

fn main() {}
