// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

contract UnmatchedInlineConfigTest is DSTest {
    // A test-named function that is not externally callable never runs as a
    // test, so its directive cannot take effect; the suite reports a warning
    // instead of silently ignoring it.
    /// forge-config: default.fuzz.runs = 15
    function testFuzz_NotExternallyCallable(uint256) internal pure {}

    function test_Runs() public {
        assertTrue(true);
    }
}
