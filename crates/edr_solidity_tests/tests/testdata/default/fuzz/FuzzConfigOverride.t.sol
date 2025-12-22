// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract FuzzConfigOverrideTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    // Test runs with overridden config (runs = 10).
    function testFuzz_OverrideRuns(uint256 a) public {
        assertEq(a, a);
    }

    // Test runs with default config (no overrides).
    function testFuzz_NoOverrideRuns(uint256 a) public {
        assertEq(a, a);
    }

    // Test times out after 1s and is marked as succeeded. max_test_rejects = 50000 to ensure it doesn't fail.
    function testFuzz_OverrideTimeoutAndRejects(uint256 a) public {
        vm.assume(a == 0);
    }

    // vm.assume(a < 0) is never true. Test has no timeout, rejects max_test_rejects = 5000 inputs and fails.
    function testFuzz_NoOverrideTimeout(uint256 a) public {
        vm.assume(a < 0);
    }

    // vm.assume(a < 0) is never true. Test rejects max_test_rejects = 0 inputs and fails immediately.
    function testFuzz_NoOverrideRejects(uint256 a) public {
        vm.assume(a < 0);
    }
}
