// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

contract FuzzRareFailureTest is DSTest {
    // Fails on about one input in 4000. Hashing the input keeps the fuzz
    // dictionary from finding the failing inputs.
    function testFuzz_rareFailure(uint256 x) public pure {
        require(uint256(keccak256(abi.encode(x))) % 4000 != 0);
    }
}
