// SPDX-License-Identifier: MIT
pragma solidity ^0.8.18;

import "ds-test/test.sol";

contract Counter {
    uint256 public number;

    // Reverts
    function setNumberReverts(uint256 newNumber) public {
        require(number > 10000000000, "low number");
        number = newNumber;
    }

    // Does not revert
    function setNumber(uint256 newNumber) public {
        number = newNumber;
    }
}

contract CounterTest is DSTest {
    Counter public counter;

    function setUp() public {
        counter = new Counter();
    }

    // Tests should pass because the revert happens in Counter contract
    function testFuzz_SetNumberRequire(uint256 x) public {
        counter.setNumberReverts(x);
        require(counter.number() == 1);
    }

    function testFuzz_SetNumberAssert(uint256 x) public {
        counter.setNumberReverts(x);
        assertEq(counter.number(), 1);
    }
}

contract AnotherCounterTest is DSTest {
    Counter public counter;

    function setUp() public {
        counter = new Counter();
    }

    // Tests should fail because the revert happens in the test itself
    function testFuzz_SetNumberRequire(uint256 x) public {
        counter.setNumber(x);
        require(counter.number() == 1);
    }

    function testFuzz_SetNumberAssert(uint256 x) public {
        counter.setNumber(x);
        assertEq(counter.number(), 1);
    }
}