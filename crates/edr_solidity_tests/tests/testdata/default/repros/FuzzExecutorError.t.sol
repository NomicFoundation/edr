// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

// Fixture for `fuzz_executor_error_reports_no_stack_trace`: the fuzzed call
// reads an account that only exists behind the fork's RPC, which the test's
// mock server fails on purpose — an executor-level error rather than a
// revert, so the counterexample records no trace arena.
contract FuzzExecutorErrorTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function setUp() public {
        vm.createSelectFork("mock");
    }

    function testFuzzTouchesUnfetchableAccount(uint256) public {
        require(address(0xdEaD).balance == 0, "unreachable: the fetch fails");
    }
}
