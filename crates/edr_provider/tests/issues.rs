// Each Cargo integration test is a separate binary, so just `mod issues;` would
// point to us (the binary crate) in a circular fashion. Instead of using
// a different name for the actual tests, we work around it with a `#[path]`
// attribute for clarity.
#[path = "issues/mod.rs"]
mod issues;

fn main() {}
