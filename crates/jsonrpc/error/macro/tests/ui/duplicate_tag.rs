use edr_jsonrpc_error_macro::rpc_error;

#[rpc_error(tag = "first", tag = "second")]
pub struct Bad {
    x: String,
}

fn main() {}
