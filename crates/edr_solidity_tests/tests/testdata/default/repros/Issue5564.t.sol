// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";
import {console} from "../logs/console.sol";

// https://github.com/foundry-rs/foundry/issues/5564
contract Issue5564Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    error MyError();

    function testSelfMeteringRevert() public {
        vm.pauseGasMetering();
        vm.expectRevert(MyError.selector);
        this.selfReverts();
    }

    function selfReverts() external {
        vm.resumeGasMetering();
        revert MyError();
    }
}
