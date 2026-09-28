// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import {Test} from "forge-std/src/Test.sol";

contract InlineConfigProfilesTest is Test {
    // No profile prefix, so it applies under every profile (runs = 3).
    /// hardhat-config: fuzz.runs = 3
    function testFuzz_Unprefixed(uint256 a) public pure {
        assertEq(a, a);
    }

    // Under `ci` the prefixed directive wins, even though it's written first.
    /// hardhat-config: ci.fuzz.runs = 8
    /// hardhat-config: fuzz.runs = 3
    function testFuzz_ProfileWinsOverUnprefixed(uint256 a) public pure {
        assertEq(a, a);
    }

    // Applies only under `default`; other profiles use the global config.
    /// hardhat-config: default.fuzz.runs = 4
    function testFuzz_DefaultProfileOnly(uint256 a) public pure {
        assertEq(a, a);
    }
}
