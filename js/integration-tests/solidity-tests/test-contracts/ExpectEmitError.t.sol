// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";

contract Counter {
    uint public x;

    event Increment(uint by);

    function inc() public {
        x++;
        emit Increment(1);
    }

    function incBy(uint by) public {
        require(by < 0, "incBy: increment should be positive");
        x += by;
        emit Increment(by);
    }
}

contract ExpectEmitErrorTest is Test {
    Counter counter;

    function setUp() public {
        counter = new Counter();
    }

    function testExpectEmitShouldFail() public {
        Counter counter = new Counter();

        vm.expectEmit(address(counter));
        emit Counter.Increment(2);

        // Emits `Counter.Increment(3)`, so `expectEmit` should fail
        counter.incBy(3);
    }

    function testExpectEmitShouldSucceed() public {
        Counter counter = new Counter();

        vm.expectEmit(address(counter));
        emit Counter.Increment(2);

        // Emits `Counter.Increment(2)`, so `expectEmit` should succeed
        counter.incBy(2);
    }
}