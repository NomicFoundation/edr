use edr_jsonrpc_error_macro::rpc_error;

// cfg_attr with an always-true predicate is equivalent to applying the
// inner attribute directly. We reject serde(transparent) regardless of
// how it's wrapped.
#[rpc_error(tag = "bad")]
#[cfg_attr(all(), serde(transparent))]
pub struct Bad {
    x: String,
}

fn main() {}
