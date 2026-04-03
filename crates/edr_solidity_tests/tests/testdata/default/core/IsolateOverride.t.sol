// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract Counter {
    uint256 public count;

    function increment() public {
        count += 1;
    }
}

contract IsolateOverrideTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    /// This test only passes with isolate = true because nonces only increment
    /// per external call in isolation mode.
    function testNonceIncrementsWithIsolation() public {
        address bob = address(14);
        vm.startPrank(bob);
        Counter counter = new Counter();
        assertEq(vm.getNonce(bob), 1);
        counter.increment();
        assertEq(vm.getNonce(bob), 2);
    }
}
