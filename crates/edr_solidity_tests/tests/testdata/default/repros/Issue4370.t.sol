// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract Issue4370Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    uint a;
    function test_negativeGas () public {
        vm.pauseGasMetering();
        a = 100;
        vm.resumeGasMetering();
        delete a;
    }
}