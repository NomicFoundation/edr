// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract StateGasTarget {
    uint256 public slot;

    function write(uint256 value) external {
        slot = value;
    }

    function fail() external pure {
        revert("failed");
    }
}

/// EIP-8037 state gas as seen from the Solidity test runner. Only meaningful on Amsterdam.
contract Eip8037StateGasTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    // A fresh slot costs `STATE_BYTES_PER_STORAGE_SET * CPSB` = 64 * 1530 of state gas.
    uint64 constant FRESH_SLOT_STATE_GAS = 97_920;

    function testFreshSlotWriteReportsOneSlotOfStateGas() public {
        StateGasTarget target = new StateGasTarget();
        target.write(1);

        Vm.Gas memory gas = vm.lastFrameGas();
        assertEq(gas.gasStateUsed, int64(FRESH_SLOT_STATE_GAS));
        assertEq(vm.lastCallGas().gasStateUsed, int64(FRESH_SLOT_STATE_GAS));
    }

    function testCreateFrameReportsStateGas() public {
        new StateGasTarget();

        // Account creation and code deposit are state gas.
        assertGt(uint256(int256(vm.lastFrameGas().gasStateUsed)), 0);
    }

    function testRevertedFrameReportsZeroStateGas() public {
        StateGasTarget target = new StateGasTarget();
        try target.fail() {} catch {}

        assertEq(vm.lastFrameGas().gasStateUsed, int64(0));
    }

    // Independent of the gas cheatcodes: under EIP-8037 the regular gas charged for a fresh
    // slot write drops to a few thousand, since the state part is paid from the reservoir.
    function testAmsterdamRepricesRegularGasOfFreshSlotWrite() public {
        StateGasTarget target = new StateGasTarget();

        uint256 before = gasleft();
        target.write(1);
        uint256 delta = before - gasleft();

        assertLt(delta, 10_000);
    }
}
