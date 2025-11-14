// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract Counter {
    uint256 public number;
    error NumberNotEven(uint256 number);
    error RandomError();
    function setNumber(uint256 newNumber) public {
        if (newNumber % 2 != 0) {
            revert NumberNotEven(newNumber);
        }
        number = newNumber;
    }
}
contract Issue8705Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    Counter public counter;
    function setUp() public {
        counter = new Counter();
        counter.setNumber(0);
    }
    function test_decode() public {
        vm.expectRevert(Counter.RandomError.selector);
        counter.setNumber(1);
    }
    function test_decode_with_args() public {
        vm.expectRevert(abi.encodePacked(Counter.NumberNotEven.selector, uint(2)));
        counter.setNumber(1);
    }
}