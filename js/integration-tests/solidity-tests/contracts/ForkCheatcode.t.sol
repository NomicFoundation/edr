// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";
import {console} from "forge-std/src/console.sol";

// Test that the fork cheatcode works correctly
contract ForkCheatcodeTest is Test {
    uint256 fork;

    function setUp() public {
        fork = vm.createSelectFork("alchemyMainnet", 20_000_000);
    }

    function testBlockNumber() public {
        assertEq(fork, vm.activeFork());
        assertEq(block.number, 20_000_000);
    }
}

contract ForkCheatcodeStacktraceTest is Test {
    function testBlockNumberInStackTrace() public {
        uint start = vm.unixTime();
        // Implicit latest block number
        uint256 fork = vm.createSelectFork("alchemyMainnet");
        uint end = vm.unixTime();
        // Log the block number so we know what was the block number during the first execution
        console.log(block.number);
        // If creating the new fork took less than 5 milliseconds, then it didn't create a new fork which needs RPC calls.
        // This is the expected outcome during re-execution, so don't sleep during re-execution.
        if (5 <= end - start) {
            // Wait before reverting reverting so that there is a new block when re-execution happens
            vm.sleep(13_000);
        }
        // Revert with the block number so that we know what was the block number during re-execution for stack traces.
        revert(vm.toString(block.number));
    }
}
