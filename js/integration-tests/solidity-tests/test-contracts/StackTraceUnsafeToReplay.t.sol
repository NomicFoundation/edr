// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import {Test} from "forge-std/src/Test.sol";

contract StackTraceUnsafeToReplay is Test {
    function testThatFails() public view {
        string memory key = "_EDR_SOLIDITY_TESTS_GET_ENV_TEST_KEY";
        // Use an impure cheatcode to make the test unsafe to replay
        string memory result = vm.envString(key);

        assertEq(result, "InvalidEqualityValue");
    }
}
