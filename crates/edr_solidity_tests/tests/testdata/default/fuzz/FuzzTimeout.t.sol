// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract FuzzTimeoutTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function test_fuzz_bound(uint256 a) public pure {
        vm.assume(a == 0);
    }
}