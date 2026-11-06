// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract FuzzWithRejectsTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testFuzzWithRejects(uint256 x) public pure {
        vm.assume(x < 1_000_000);
    }
}
