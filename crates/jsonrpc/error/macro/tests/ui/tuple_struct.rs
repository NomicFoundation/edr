use edr_jsonrpc_error_macro::rpc_error;

#[rpc_error(tag = "bad")]
pub struct BadTuple(String, u64);

fn main() {}
