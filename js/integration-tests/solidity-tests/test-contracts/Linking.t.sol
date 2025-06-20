// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";
import "../contracts/MyLibrary.sol";

contract LibraryConsumer {
    function consume(uint256 a) public pure returns (uint256) {
        return MyLibrary.plus100(a);
    }
}

contract LinkingTest is Test {
    LibraryConsumer consumer;


    function setUp() public {
        consumer = new LibraryConsumer();
    }

    function testCallLibrary() public view {
        assertEq(consumer.consume(1), 101, "library call failed");
    }
}
