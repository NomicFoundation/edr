// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import {Test} from "forge-std/src/Test.sol";

contract CallTracesFailingOnlySetup is Test {
    uint256 public initialValue;

    function setUp() public {
        initialValue = 42;
    }

    function testSuccessfulTest() public view {
        require(initialValue == 42, "Setup not called properly");
    }

    function testIntentionallyFailingTest() public pure {
        revert("This test intentionally fails");
    }
}
