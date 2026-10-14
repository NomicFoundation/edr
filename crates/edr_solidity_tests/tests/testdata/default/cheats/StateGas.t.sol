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

    function setValueWithData(bytes calldata) external {
        value = 1;
    }
}

contract LargeRuntime {
    constructor() {
        assembly {
            return(0, 12000)
        }
    }
}

contract StateGasRefundTarget {
    uint256 value;
    uint256 cleared = 1;

    function setValueAndClear() external {
        value = 1;
        cleared = 0;
    }
}

/// forge-config: default.evm_version = "Amsterdam"
contract StateGasTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    // EIP-8037: a new storage slot costs 64 state bytes at 1530 gas each.
    int64 constant STORAGE_SET_STATE_GAS = 64 * 1530;
    // EIP-8037: deployed code costs one state byte per runtime byte.
    int64 constant LARGE_RUNTIME_STATE_GAS = 12_000 * 1530;
    // EIP-8037: creating an account costs 120 state bytes.
    uint256 constant CREATE_STATE_GAS = 120 * 1530;
    uint256 constant CALLDATA_SIZE = 20_000;
    // EIP-7976 calldata floor (Amsterdam): the base transaction gas plus 64 gas per calldata byte.
    // TODO(revm 42): 15_000 once EIP-2780 lowers the base transaction gas.
    uint256 constant CALL_FLOOR_BASE_GAS = 21_000;
    uint256 constant CALLDATA_FLOOR_GAS_PER_BYTE = 64;

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

    /// forge-config: default.isolate = true
    function testCalldataFloorPreservesRegularGas() public {
        bytes memory data = new bytes(CALLDATA_SIZE);
        StateGasTarget target = new StateGasTarget();
        bytes memory callData = abi.encodeCall(target.setValueWithData, (data));
        uint256 floorGas = CALL_FLOOR_BASE_GAS + callData.length * CALLDATA_FLOOR_GAS_PER_BYTE;
        (bool success,) = address(target).call(callData);
        assertTrue(success, "state gas call failed");

        // The isolated transaction's regular gas is the calldata floor; state gas is separate.
        Vm.Gas memory gas = vm.lastFrameGas();
        assertEq(gas.gasTotalUsed, floorGas, "wrong calldata floor gas");
        assertEq(gas.gasStateUsed, STORAGE_SET_STATE_GAS, "wrong state gas");
    }

    function testLargeContractDeployment() public {
        LargeRuntime deployed = new LargeRuntime();
        assertEq(address(deployed).code.length, 12_000);
        assertEq(vm.lastFrameGas().gasStateUsed, LARGE_RUNTIME_STATE_GAS, "wrong code deposit state gas");
    }

    /// forge-config: default.isolate = true
    function testIsolatedLargeContractDeployment() public {
        LargeRuntime deployed = new LargeRuntime();
        assertEq(address(deployed).code.length, 12_000);
        // The CREATE opcode already charged the account creation outside the frame.
        Vm.Gas memory gas = vm.lastFrameGas();
        assertEq(gas.gasStateUsed, LARGE_RUNTIME_STATE_GAS, "wrong code deposit state gas");
        // The deployment's receipt also includes the account creation state gas.
        assertEq(
            vm.snapshotGasLastFrame("stateGasCreate"),
            gas.gasTotalUsed + uint64(gas.gasStateUsed) + CREATE_STATE_GAS,
            "create snapshot is not the receipt gas"
        );
    }

    /// forge-config: default.isolate = true
    function testStateGasSnapshotIncludesRefund() public {
        StateGasRefundTarget target = new StateGasRefundTarget();
        target.setValueAndClear();

        Vm.Gas memory gas = vm.lastCallGas();
        assertEq(gas.gasStateUsed, STORAGE_SET_STATE_GAS, "wrong state gas");
        assertGt(gas.gasRefunded, 0, "storage clear was not refunded");
        assertEq(
            vm.snapshotGasLastCall("stateGasRefund"),
            gas.gasTotalUsed + uint64(gas.gasStateUsed) - uint64(gas.gasRefunded),
            "snapshot ignored the refund"
        );
    }

    /// forge-config: default.isolate = true
    function testValueTransferWithLowGas() public {
        uint256 section = snapshotValueTransfer(address(0xBEEF1), 1_000_000);
        uint256 snapshot = vm.snapshotGasLastCall("valueTransfer");

        // Creating the recipient charges state gas, which must not turn a low gas limit or the
        // 2300 gas stipend into a zero snapshot or a region refund.
        assertEq(snapshotValueTransfer(address(0xBEEF2), 50_000), section, "low gas region changed");
        assertEq(vm.snapshotGasLastCall("lowGasValueTransfer"), snapshot, "low gas snapshot changed");
        assertEq(snapshotValueTransfer(address(0xBEEF3), 0), section, "stipend region changed");
        assertEq(vm.snapshotGasLastCall("stipendValueTransfer"), snapshot, "stipend snapshot changed");
    }

    /// forge-config: default.isolate = true
    function testCalldataFloorWithLowGas() public {
        bytes memory data = new bytes(CALLDATA_SIZE);
        uint256 floorGas = CALL_FLOOR_BASE_GAS + data.length * CALLDATA_FLOOR_GAS_PER_BYTE;
        // The calldata floor exceeds both the forwarded gas and the intrinsic gas.
        (bool success,) = address(0xCA11).call{gas: 1_000}(data);
        assertTrue(success, "low gas call failed");
        assertEq(vm.snapshotGasLastCall("calldataFloorLowGas"), floorGas, "wrong calldata floor gas");
    }

    function snapshotValueTransfer(address to, uint256 gasLimit) internal returns (uint256 section) {
        vm.startSnapshotGas("valueTransferSection");
        (bool success,) = to.call{value: 1, gas: gasLimit}("");
        section = vm.stopSnapshotGas();
        assertTrue(success, "value transfer failed");
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
