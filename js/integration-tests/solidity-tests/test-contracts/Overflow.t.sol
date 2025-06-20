// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

// Contract to be tested with overflow vulnerability
contract MyContract {
    function addWithOverflow(uint256 a, uint256 b) public pure returns (uint256) {
        return a + b;
    }
}

// Test that the fuzzing catches overflows
contract OverflowTest is Test {
    MyContract public myContract;

    function setUp() public {
        myContract = new MyContract();
    }

    function testFuzzAddWithOverflow(uint256 a, uint256 b) public view {
        uint256 result = myContract.addWithOverflow(a, b);
        assertEq(result, a + b);
    }
}
