// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

// Fixture for `always_mode_produces_stack_trace_for_failing_test`: a failing
// test that has a `setUp`, and reverts inside a called contract.
contract AlwaysStackTraceTest is DSTest {
    Reverter reverter;
    uint256[] public fixtureAmount = [1];

    function setUp() public {
        reverter = new Reverter();
    }

    // `test`-prefixed so the revert counts as a failure (triggers a stack trace).
    function testRevertHasStackTrace() public {
        reverter.boom();
    }

    // `table`-prefixed with a param => runs as a table test (fed by
    // `fixtureAmount`), covering the table-test stack-trace path.
    function tableRevertHasStackTrace(uint256 amount) public {
        require(amount == 1, "amount");
        reverter.boom();
    }
}

contract Reverter {
    function boom() public pure {
        require(false, "boom");
    }
}
