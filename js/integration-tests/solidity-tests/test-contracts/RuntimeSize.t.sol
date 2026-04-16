// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {Greeter} from "../contracts/Greeter.sol";

contract RuntimeSizeShortTest is Test {
    Greeter public greeter;

    function setUp() public {
        greeter = new Greeter("Hi");
    }

    function test_greet() public view {
        assertEq(greeter.greeting(), "Hi");
    }
}

contract RuntimeSizeLongTest is Test {
    Greeter public greeter;

    function setUp() public {
        greeter = new Greeter("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    }

    function test_greet() public view {
        assertEq(greeter.greeting(), "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    }
}
