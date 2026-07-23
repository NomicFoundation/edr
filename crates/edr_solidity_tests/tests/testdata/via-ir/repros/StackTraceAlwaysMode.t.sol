// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

// Fixture for `always_mode_produces_stack_trace_for_failing_test`: a failing
// test with a `setUp` that reverts inside a called contract.
contract AlwaysStackTraceTest is DSTest {
    Reverter reverter;

    function setUp() public {
        reverter = new Reverter();
    }

    // `test`-prefixed so the revert counts as a failure (triggers a stack trace).
    function testRevertHasStackTrace() public {
        reverter.boom();
    }
}

contract Reverter {
    function boom() public pure {
        require(false, "boom");
    }
}
