// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract CounterWithRevert {
    error CountError();
    error CheckError();

    function count(uint256 a) public pure returns (uint256) {
        if (a > 1000 || a < 10) {
            revert CountError();
        }
        return 99999999;
    }
    function check(uint256 a) public pure {
        if (a == 99999999) {
            revert CheckError();
        }
    }
    function dummy() public pure {}
}

// `test_assume_no_revert` in `crates/forge/tests/cli/test_cmd.rs`
contract AssumeNoRevertTest is DSTest {
    Vm vm = Vm(HEVM_ADDRESS);

    function test_assume_no_revert_pass(uint256 a) public {
        CounterWithRevert counter = new CounterWithRevert();
        vm.assumeNoRevert();
        a = counter.count(a);
        assertEq(a, 99999999);
    }
    function test_assume_no_revert_fail_assert(uint256 a) public {
        CounterWithRevert counter = new CounterWithRevert();
        vm.assumeNoRevert();
        a = counter.count(a);
        // Test should fail on next assertion.
        assertEq(a, 1);
    }
    function test_assume_no_revert_fail_in_2nd_call(uint256 a) public {
        CounterWithRevert counter = new CounterWithRevert();
        vm.assumeNoRevert();
        a = counter.count(a);
        // Test should revert here (not in scope of `assumeNoRevert` cheatcode).
        counter.check(a);
        assertEq(a, 99999999);
    }
    function test_assume_no_revert_fail_in_3rd_call(uint256 a) public {
        CounterWithRevert counter = new CounterWithRevert();
        vm.assumeNoRevert();
        a = counter.count(a);
        // Test `assumeNoRevert` applied to non reverting call should not be available for next reverting call.
        vm.assumeNoRevert();
        counter.dummy();
        // Test will revert here (not in scope of `assumeNoRevert` cheatcode).
        counter.check(a);
        assertEq(a, 99999999);
    }
}