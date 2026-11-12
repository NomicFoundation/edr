// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

// The project's own `Permit`. It clashes by name with the vendored copy in
// `vendor/Permit.sol`, which is what `eip712ExcludeGlobs` settles.
struct Permit {
    address owner;
    address spender;
    uint256 value;
    uint256 nonce;
    uint256 deadline;
}
