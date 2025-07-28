// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

contract InternalExpectRevertTest is Test {
    function testInternalExpectRevert() public {
        vm.expectRevert();
        revert("reverted in top level scope");
    }
}
