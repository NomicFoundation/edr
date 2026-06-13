// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

// Imported via a relative import by Eip712LazyTest.t.sol, to exercise lazy
// EIP-712 type collection across relative imports.
struct Asset {
    address token;
    uint256 amount;
}
