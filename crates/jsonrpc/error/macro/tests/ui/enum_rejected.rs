use edr_jsonrpc_error_macro::rpc_error;

#[rpc_error(tag = "bad")]
pub enum BadEnum { A, B { x: u32 } }

fn main() {}
