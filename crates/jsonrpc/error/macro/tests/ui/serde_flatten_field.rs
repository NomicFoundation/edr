use edr_jsonrpc_error_macro::rpc_error;

#[derive(serde::Serialize)]
struct Inner { a: u32 }

#[rpc_error(tag = "bad")]
pub struct Bad {
    #[serde(flatten)]
    inner: Inner,
}

fn main() {}
