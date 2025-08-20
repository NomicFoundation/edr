//! Solidity ABI-related utilities and [`sol!`](alloy_sol_types::sol) definitions.

pub use foundry_cheatcodes_spec::Vm;

pub mod console;
mod test_function_ext;
pub use test_function_ext::TestFunctionExt;
