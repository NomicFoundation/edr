// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import {Test} from "forge-std/src/Test.sol";

contract CallTracesSetup is Test {
    uint256 public initialValue;

    function setUp() public {
        initialValue = 42;
        emit SetupEvent("setUp called", initialValue);
    }

    function testAfterSetup() public {
        require(initialValue == 42, "Setup not called properly");
        emit TestEvent("test after setup", initialValue);
    }

    event SetupEvent(string message, uint256 value);
    event TestEvent(string message, uint256 value);
}
