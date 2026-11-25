// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

// A vendored `Permit` declaring different members than the project's own, so
// the name resolves to neither until one of the two files is excluded.
struct Permit {
    address owner;
    address spender;
    uint256 value;
    uint256 deadline;
}
