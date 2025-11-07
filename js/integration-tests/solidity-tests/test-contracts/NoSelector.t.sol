// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";

contract TestHandler is Test {}

contract NoSelectorTest is Test {
    bytes4[] selectors;

    function setUp() public {
        TestHandler handler = new TestHandler();
        targetSelector(FuzzSelector({addr: address(handler), selectors: selectors}));
        excludeSelector(FuzzSelector({addr: address(handler), selectors: selectors}));
    }

    function invariant_panic() public {}
}