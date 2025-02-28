// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

// Test that the fork cheatcode works correctly
contract ForkCheatcodeTest is Test {
    uint256 fork;

    function setUp() public {
        fork = vm.createSelectFork("alchemyMainnet", 20_000_000);
    }

    function testBlockNumber() public view {
        assertEq(fork, vm.activeFork());
        assertEq(block.number, 20_000_000);
    }
}

contract LatestForkCheatcodeTest is Test {
    uint256 fork;

    function setUp() public {
        fork = vm.createSelectFork("alchemyMainnet");
    }

    function testThatFails() public view {
        revert("fail");
    }
}
