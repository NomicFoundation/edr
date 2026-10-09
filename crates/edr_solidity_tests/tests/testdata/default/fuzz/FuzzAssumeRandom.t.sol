// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract FuzzAssumeRandomTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    // Rejects based on the cheatcode RNG alone. If a rejected run were retried
    // with the same RNG seed, it would be rejected again, forever.
    function testFuzz_assumeRandom(uint256) public {
        vm.assume(vm.randomUint(0, 1) == 0);
    }
}
