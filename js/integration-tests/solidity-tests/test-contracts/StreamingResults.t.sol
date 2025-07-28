// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

// Test that test suite results are returned in the order of completion and immediately after they're done.

contract FirstReturnTest is Test {
    function testThatSucceedsImmediately() public {
        vm.sleep(50); // ms
        require(1 == 1, "1 is not equal to 1");
    }
}

contract SecondReturnTest is Test {
    function testThatSucceedsImmediately() public {
        vm.sleep(250); // ms
        require(1 == 1, "1 is not equal to 1");
    }
}

contract ThirdReturnTest is Test {
    function testThatSucceedsImmediately() public {
        vm.sleep(750); // ms
        require(1 == 1, "1 is not equal to 1");
    }
}
