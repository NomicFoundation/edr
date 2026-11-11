// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

// Inline overrides apply before the check that a disabled reject limit comes
// with a timeout.
contract FuzzNoRejectLimitOverrideTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    // Disables the reject limit without a timeout; rejected under any config.
    /// forge-config: default.fuzz.max-test-rejects = 0
    function testFuzz_InlineNoRejectLimit(uint256 a) public pure {
        vm.assume(a == 0);
    }

    // Times out after 1s and succeeds when the config disables the reject
    // limit.
    /// forge-config: default.fuzz.timeout = 1
    function testFuzz_InlineTimeout(uint256 a) public pure {
        vm.assume(a == 0);
    }
}
