// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

contract FuzzProfileOverrideTest is DSTest {
    // No profile prefix, so it applies under every profile (runs = 3).
    /// forge-config: fuzz.runs = 3
    function testFuzz_Unprefixed(uint256 a) public {
        assertEq(a, a);
    }

    // Under `ci` the prefixed directive wins, even though it's written first.
    /// forge-config: ci.fuzz.runs = 8
    /// forge-config: fuzz.runs = 3
    function testFuzz_ProfileWinsOverUnprefixed(uint256 a) public {
        assertEq(a, a);
    }

    // Applies only under `default`; other profiles use the global config.
    /// forge-config: default.fuzz.runs = 4
    function testFuzz_DefaultProfileOnly(uint256 a) public {
        assertEq(a, a);
    }
}
