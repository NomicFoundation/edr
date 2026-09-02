// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

// Fixture for `test_stack_trace_re_run_applies_overrides_after_setup`: the
// suite runs at a hardfork with PUSH0, which `setUp()` executes, while the
// failing test selects Merge (Paris), which predates PUSH0, through its
// inline `evmVersion` directive. The re-run that computes the stack trace
// must apply that override only after `setUp()`, as the original run does,
// or `setUp()` fails in the re-run.
contract OverrideAfterSetup is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function setUp() public {
        address target = address(uint160(uint256(0xc4f3)));
        // 5F PUSH0, 5F PUSH0, F3 RETURN: returns empty data on Shanghai and
        // above; PUSH0 is an invalid opcode before that.
        vm.etch(target, hex"5f5ff3");
        (bool success, ) = target.call("");
        require(success, "PUSH0 unsupported");
    }

    /// forge-config: default.evmVersion = "Merge"
    function testRevertsAtMerge() public pure {
        require(false, "boom");
    }

    // The fuzz counterpart, so the fuzz counterexample re-run is covered too.
    /// forge-config: default.evmVersion = "Merge"
    function testFuzzRevertsAtMerge(uint256) public pure {
        require(false, "boom");
    }
}
