// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import {Test} from "forge-std/src/Test.sol";

contract InternalRevertingTest is Test {
    // Passes with allow_internal_expect_revert = true
    function testInternalRevert() public {
        vm.expectRevert("should revert here");
        require(false, "should revert here");
    }
}
