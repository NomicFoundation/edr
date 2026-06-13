// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

// Imported via a non-relative ("mapped") import by Eip712LazyTest.t.sol, to
// exercise the `eip712ImportMappings` config that maps import paths to disk
// paths.
struct Coupon {
    uint256 id;
    address issuer;
}
