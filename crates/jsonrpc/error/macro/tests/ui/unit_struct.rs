use edr_jsonrpc_error_macro::rpc_error;

#[rpc_error(tag = "bad")]
pub struct BadUnit;

fn main() {}
