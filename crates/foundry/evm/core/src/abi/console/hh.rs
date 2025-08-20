//! Hardhat `console.sol` interface.

use alloy_sol_types::sol;
use edr_macros::ConsoleFmt;

sol!(
    #[sol(abi)]
    #[derive(ConsoleFmt)]
    Console,
    "src/abi/Console.json"
);

pub use Console::*;
