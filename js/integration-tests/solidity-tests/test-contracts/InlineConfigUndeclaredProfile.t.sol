// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import {Test} from "forge-std/src/Test.sol";

// The directive names a profile the run doesn't declare, so the inline-config
// parser rejects it and aborts the whole run before any test executes.
contract InlineConfigUndeclaredProfileTest is Test {
    /// hardhat-config: nope.fuzz.runs = 1
    function testFuzz_UndeclaredProfile(uint256 a) public pure {
        assertEq(a, a);
    }
}
