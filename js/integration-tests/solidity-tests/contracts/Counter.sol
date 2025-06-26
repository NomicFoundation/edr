// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

contract Counter {
    uint256 public number;

    function setNumber(uint256 newNumber) public {
        number = newNumber;
    }

    function increment() public {
        number++;
    }

    function decrement() public {
        number--;
    }

    function getNumber() public view returns (uint256) {
        return number;
    }
}

