// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract RevertingTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testRevert() public {
        vm._expectInternalRevert();
        require(false, "should revert here");
    }
}
