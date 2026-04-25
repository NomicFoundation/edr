use edr_jsonrpc_error_macro::rpc_error;

#[rpc_error(name = "not-a-tag")]
pub struct Bad {
    x: String,
}

fn main() {}
