// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./test.sol";
import "./Vm.sol";

contract FailingSetupTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function setUp() public {
        vm.createSelectFork("nonExistentForkAlias", 20_000_000);
    }
}
