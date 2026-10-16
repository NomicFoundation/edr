// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract StateGasTarget {
    uint256 value;

    function setValue() external {
        value = 1;
    }

    function setValueAndRevert() external {
        value = 1;
        revert();
    }
}

/// forge-config: default.evm_version = "Amsterdam"
contract StateGasTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    // EIP-8037: a new storage slot costs 64 state bytes at 1530 gas each.
    int64 constant STORAGE_SET_STATE_GAS = 64 * 1530;

    function testReportsStateGas() public {
        StateGasTarget target = new StateGasTarget();
        target.setValue();

        assertStorageWriteGas(vm.lastCallGas());
        assertStorageWriteGas(vm.lastFrameGas());

        StateGasTarget revertingTarget = new StateGasTarget();
        (bool success,) = address(revertingTarget).call(abi.encodeCall(revertingTarget.setValueAndRevert, ()));
        assertTrue(!success, "state gas call should revert");
        assertEq(vm.lastFrameGas().gasStateUsed, 0, "reverted frame used state gas");
    }

    function assertStorageWriteGas(Vm.Gas memory gas) internal {
        assertGt(gas.gasTotalUsed, 0, "regular gas was not recorded");
        assertLt(gas.gasTotalUsed, uint64(STORAGE_SET_STATE_GAS), "state gas counted as regular gas");
        assertEq(gas.gasStateUsed, STORAGE_SET_STATE_GAS, "wrong state gas");
    }
}

contract PreAmsterdamStateGasTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testNoStateGasBeforeAmsterdam() public {
        StateGasTarget target = new StateGasTarget();
        target.setValue();

        assertEq(vm.lastFrameGas().gasStateUsed, 0, "state gas before Amsterdam");
    }
}
