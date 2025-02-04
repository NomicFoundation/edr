// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

contract FailingSetupTest is Test {
    function setUp() public {
        vm.createSelectFork("nonExistentForkAlias", 20_000_000);
    }
}
