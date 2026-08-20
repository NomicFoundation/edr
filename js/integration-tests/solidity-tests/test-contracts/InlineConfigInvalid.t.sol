// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import {Test} from "forge-std/src/Test.sol";

// Each test function carries a distinct malformed inline-config directive. The
// directives are only comments, so the contract compiles; the inline-config
// parser rejects them and aborts the whole run before any test executes.
contract InlineConfigInvalidTest is Test {
    /// hardhat-config: default.fuzz.runs = -1
    function testFuzz_InvalidRuns(uint256 a) public pure {
        assertEq(a, a);
    }

    /// hardhat-config: default.fuzz.bogusKey = 1
    function testFuzz_InvalidKey(uint256 a) public pure {
        assertEq(a, a);
    }
}
