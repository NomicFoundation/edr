// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

contract Counter {
    uint256 public number;

    function increment() public {
        number++;
    }
}


contract CounterTableTest is DSTest {
    Counter counter = new Counter();

    uint256[] public fixtureAmount = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    bool[] public fixtureSwap = [true, true, false, true, false, true, false, true, false, true];
    bool[] public fixtureDiffSwap = [true, false];
    function fixtureNoFixture() public returns (address[] memory) {
    }

    function tableWithNoParamFail() public {
        counter.increment();
    }

    function tableWithParamNoFixtureFail(uint256 noFixture) public {
        require(noFixture != 100);
        counter.increment();
    }

    function tableSingleParamPass(uint256 amount) public {
        require(amount != 100, "Amount cannot be 100");
        counter.increment();
    }

    function tableSingleParamFail(uint256 amount) public {
        require(amount != 10, "Amount cannot be 10");
        counter.increment();
    }

    function tableMultipleParamsNoParamFail(uint256 amount, bool noSwap) public {
        require(amount != 100 && noSwap, "Amount cannot be 100");
        counter.increment();
    }

    function tableMultipleParamsDifferentFixturesFail(uint256 amount, bool diffSwap) public {
        require(amount != 100 && diffSwap, "Amount cannot be 100");
        counter.increment();
    }

    function tableMultipleParamsFail(uint256 amount, bool swap) public {
        require(amount == 3 && swap, "Cannot swap");
        counter.increment();
    }

    function tableMultipleParamsPass(uint256 amount, bool swap) public {
        if (amount == 3 && swap) {
            revert();
        }
        counter.increment();
    }
}