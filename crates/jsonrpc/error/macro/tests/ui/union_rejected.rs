use edr_jsonrpc_error_macro::rpc_error;

#[rpc_error(tag = "bad")]
pub union BadUnion {
    a: u32,
    b: u64,
}

fn main() {}
