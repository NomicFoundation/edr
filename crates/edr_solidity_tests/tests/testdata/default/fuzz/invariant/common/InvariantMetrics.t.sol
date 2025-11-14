// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract CounterTest is DSTest {
    function setUp() public {
        CounterHandler handler = new CounterHandler();
    }

    function invariant_counter() public {}
}

contract CounterHandler is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function doSomething(uint256 a) public {
        vm.assume(a < 10_000_000);
        require(a < 100_000);
    }

    function doAnotherThing(uint256 a) public {
        vm.assume(a < 10_000_000);
        require(a < 100_000);
    }
}