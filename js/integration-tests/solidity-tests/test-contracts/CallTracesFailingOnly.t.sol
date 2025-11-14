// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import {Test} from "forge-std/src/Test.sol";

contract CallTracesFailingOnly is Test {
    function testSuccessfulTest() public {
    }

    function testIntentionallyFailingTest() public pure {
        revert("This test intentionally fails");
    }
}
