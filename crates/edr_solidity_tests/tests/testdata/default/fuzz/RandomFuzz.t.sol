// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract RandomFuzzTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testFuzz_randomUint_shouldFail(uint256) public {
        uint256 rand = vm.randomUint(0, 4);
        assertTrue(rand != 0, "hit value 0");
    }
}
