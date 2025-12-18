// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract InternalRevertingTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    // Passes with allow_internal_expect_revert = true
    function testInternalRevert() public {
        vm.expectRevert("should revert here");
        require(false, "should revert here");
    }
}
