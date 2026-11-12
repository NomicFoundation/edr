// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract Target {
    uint256 public slot0;

    function expandMemory(uint256 n) public pure returns (uint256) {
        uint256[] memory arr = new uint256[](n);

        for (uint256 i = 0; i < n; i++) {
            arr[i] = i;
        }

        return arr.length;
    }

    function setValue(uint256 value) public {
        slot0 = value;
    }

    function resetValue() public {
        slot0 = 0;
    }

    function failWithInvalid() public pure {
        assembly {
            invalid()
        }
    }

    fallback() external {}
}

contract TargetCreate2 {
    uint256 public value;

    constructor(uint256 value_) {
        value = value_;
    }
}

contract RevertingTarget {
    function fail() public pure {
        revert("failed");
    }
}

contract RevertingConstructor {
    constructor() {
        revert("failed");
    }
}

contract RefundingConstructor {
    constructor(Target target) {
        target.resetValue();
    }
}

contract NestedRevertingTarget {
    RevertingTarget public target;

    constructor(RevertingTarget target_) {
        target = target_;
    }

    function fail() public view {
        target.fail();
    }
}

abstract contract LastCallGasFixture is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);
    Target public target;

    struct Gas {
        uint64 gasTotalUsed;
        uint64 gasMemoryUsed;
        int64 gasRefunded;
    }

    function testRevertNoCachedLastCallGas() public {
        vm._expectCheatcodeRevert();
        vm.lastCallGas();
    }

    function testRevertNoCachedLastFrameGas() public {
        vm._expectCheatcodeRevert();
        vm.lastFrameGas();
    }

    function testLastCallGasDoesNotRecordCreate() public {
        new Target();

        vm._expectCheatcodeRevert();
        vm.lastCallGas();
    }

    function testSnapshotGasLastCallDoesNotRecordCreate() public {
        new Target();

        vm._expectCheatcodeRevert();
        vm.snapshotGasLastCall("testSnapshotGasLastCallDoesNotRecordCreate");
    }

    function _setup() internal {
        // Cannot be set in `setUp` due to `testRevertNoCachedLastCallGas`
        // relying on no calls being made before `lastCallGas` is called.
        target = new Target();
    }

    function _performCall() internal returns (bool success) {
        (success,) = address(target).call("");
    }

    function _performRefund() internal {
        target.setValue(1);
        target.resetValue();
    }

    function _assertGas(Vm.Gas memory lhs, Gas memory rhs) internal {
        assertGt(lhs.gasLimit, 0);
        assertGt(lhs.gasRemaining, 0);
        assertEq(lhs.gasTotalUsed, rhs.gasTotalUsed);
        assertEq(lhs.gasMemoryUsed, rhs.gasMemoryUsed);
        assertEq(lhs.gasRefunded, rhs.gasRefunded);
        assertEq(lhs.gasStateUsed, 0);
    }

    function _assertGasRecorded(Vm.Gas memory gas) internal {
        assertGt(gas.gasLimit, 0);
        assertGt(gas.gasRemaining, 0);
        assertGt(gas.gasTotalUsed, 0);
        assertEq(gas.gasMemoryUsed, 0);
        assertEq(gas.gasStateUsed, 0);
    }
}

contract LastFrameGasExpectedRevertTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);
    RevertingTarget public target;

    function setUp() public {
        target = new RevertingTarget();
    }

    function testExpectedRevertCallDoesNotRecordLastFrameGas() public {
        vm.expectRevert();
        target.fail();

        vm._expectCheatcodeRevert();
        vm.lastFrameGas();
    }

    function testExpectedRevertCreateDoesNotRecordLastFrameGas() public {
        vm.expectRevert();
        new RevertingConstructor();

        vm._expectCheatcodeRevert();
        vm.lastFrameGas();
    }

    function testExpectedRevertCreateClearsCachedLastFrameGas() public {
        new Target();

        vm.expectRevert();
        new RevertingConstructor();

        vm._expectCheatcodeRevert();
        vm.lastFrameGas();
    }

    function testNestedExpectedRevertCallClearsCachedLastFrameGas() public {
        NestedRevertingTarget nestedTarget = new NestedRevertingTarget(target);

        vm.expectRevert();
        nestedTarget.fail();

        vm._expectCheatcodeRevert();
        vm.lastFrameGas();
    }

    function testSnapshotGasLastFrameExpectedRevertClearsCachedLastFrameGas() public {
        new Target();

        vm.expectRevert();
        new RevertingConstructor();

        vm._expectCheatcodeRevert();
        vm.snapshotGasLastFrame("testSnapshotGasLastFrameExpectedRevertClearsCachedLastFrameGas");
    }
}

contract LastCallGasIsolatedTest is LastCallGasFixture {
    function testRecordLastFrameGasFromCall() public {
        _setup();
        _performCall();
        _assertGas(vm.lastFrameGas(), Gas({gasTotalUsed: 21064, gasMemoryUsed: 0, gasRefunded: 0}));
    }

    function testRecordLastFrameGasFromCreate() public {
        target = new Target();
        _assertGasRecorded(vm.lastFrameGas());
    }

    function testRecordLastFrameGasFromCreate2() public {
        new TargetCreate2{salt: "salt"}(1);
        _assertGasRecorded(vm.lastFrameGas());
    }

    function testRecordLastCallGas() public {
        _setup();
        _performCall();
        _assertGas(vm.lastCallGas(), Gas({gasTotalUsed: 21064, gasMemoryUsed: 0, gasRefunded: 0}));

        _performCall();
        _assertGas(vm.lastCallGas(), Gas({gasTotalUsed: 21064, gasMemoryUsed: 0, gasRefunded: 0}));

        _performCall();
        _assertGas(vm.lastCallGas(), Gas({gasTotalUsed: 21064, gasMemoryUsed: 0, gasRefunded: 0}));
    }

    function testRecordGasRefund() public {
        _setup();
        _performRefund();
        // Isolated frames report gross regular gas; snapshots keep the post-refund receipt gas.
        _assertGas(vm.lastCallGas(), Gas({gasTotalUsed: 26180, gasMemoryUsed: 0, gasRefunded: 4800}));
        assertEq(vm.snapshotGasLastCall("isolated refund call"), 21380);
        assertEq(vm.snapshotGasLastFrame("isolated refund frame"), 21380);
    }

    function testSnapshotGasSectionRefund() public {
        _snapshotResetValue(1);
    }

    function testSnapshotGasSectionNoRefund() public {
        _snapshotResetValue(0);
    }

    function testSnapshotGasSectionAfterRefund() public {
        _setup();
        _performRefund();
        _snapshotResetValue(0);
    }

    function testSnapshotGasSectionMultipleRefunds() public {
        _setup();
        Target other = new Target();
        target.setValue(1);
        other.setValue(1);
        vm.startSnapshotGas("isolated multiple refunds");
        target.resetValue();
        other.resetValue();
        // Both finalized transaction refunds are excluded from the region.
        assertEq(vm.stopSnapshotGas(), 43648);
    }

    /// forge-config: default.evm_version = "Cancun"
    function testSnapshotGasSectionCreateRefund() public {
        _setup();
        target.setValue(1);
        // Prepare the init code before measuring so compiler-dependent copying is excluded.
        bytes memory initCode = abi.encodePacked(type(RefundingConstructor).creationCode, abi.encode(target));
        vm.startSnapshotGas("isolated create refund");
        assembly {
            pop(create(0, add(initCode, 32), mload(initCode)))
        }
        uint256 section = vm.stopSnapshotGas();
        // CREATE costs 32000 gas; the remaining 20 gas is the snapshot overhead.
        // EIP-3860 additionally charges 2 gas per init code word outside the isolated frame.
        uint256 initCodeCost = 2 * ((initCode.length + 31) / 32);
        assertEq(section, vm.snapshotGasLastFrame("isolated create frame") + 32020 + initCodeCost);
    }

    function testSnapshotGasForFailedCharge() public {
        _setup();
        (bool success,) = address(target).call{gas: 100_000}(abi.encodeCall(target.failWithInvalid, ()));
        assertEq(success, false);
        // An exceptional halt consumes the forwarded gas, and the transaction also pays its
        // intrinsic gas.
        assertEq(vm.snapshotGasLastCall("isolated failed charge call"), 121_064);
        assertEq(vm.snapshotGasLastFrame("isolated failed charge frame"), 121_064);
    }

    function testSnapshotGasLastCallWithLowGas() public {
        _setup();
        uint256 lowGasSection = _snapshotCallWithGas(1_000, "");
        // The isolated transaction still pays intrinsic gas above the forwarded gas. Snapshots
        // keep that receipt gas, while the frame consumes its whole budget.
        assertEq(vm.snapshotGasLastCall("isolated low gas call"), 21_064);
        Vm.Gas memory gas = vm.lastCallGas();
        assertEq(gas.gasLimit, 1_000);
        assertEq(gas.gasTotalUsed, 1_000);
        assertEq(gas.gasRemaining, 0);

        assertEq(_snapshotCallWithGas(1_000_000, ""), lowGasSection);
        assertEq(vm.snapshotGasLastCall("isolated high gas call"), 21_064);
    }

    function testSnapshotGasSectionRefundWithLowGas() public {
        _setup();
        bytes memory data = abi.encodeCall(target.resetValue, ());
        target.setValue(1);
        uint256 lowGasSection = _snapshotCallWithGas(15_000, data);
        uint256 lowGasSnapshot = vm.snapshotGasLastCall("isolated low gas refund call");

        target.setValue(1);
        assertEq(_snapshotCallWithGas(1_000_000, data), lowGasSection);
        assertEq(vm.snapshotGasLastCall("isolated high gas refund call"), lowGasSnapshot);
    }

    function _snapshotCallWithGas(uint256 gasLimit, bytes memory data) internal returns (uint256 section) {
        vm.startSnapshotGas("isolated call with gas");
        (bool success,) = address(target).call{gas: gasLimit}(data);
        section = vm.stopSnapshotGas();
        assertTrue(success);
    }

    function _snapshotResetValue(uint256 initialValue) internal {
        _setup();
        target.setValue(initialValue);
        vm.startSnapshotGas("isolated section");
        target.resetValue();
        uint256 section = vm.stopSnapshotGas();
        // The region overhead is the same for refunding and non-refunding calls.
        assertEq(section, vm.snapshotGasLastCall("isolated section call") + 543);
    }
}

// Without isolation mode enabled the gas usage will be incorrect.
contract LastCallGasDefaultTest is LastCallGasFixture {
    function testRecordLastFrameGasFromCall() public {
        _setup();
        _performCall();
        _assertGas(vm.lastFrameGas(), Gas({gasTotalUsed: 64, gasMemoryUsed: 0, gasRefunded: 0}));
    }

    function testRecordLastFrameGasFromCreate() public {
        target = new Target();
        _assertGasRecorded(vm.lastFrameGas());
    }

    function testRecordLastFrameGasFromCreate2() public {
        new TargetCreate2{salt: "salt"}(1);
        _assertGasRecorded(vm.lastFrameGas());
    }

    function testRecordLastCallGas() public {
        _setup();
        _performCall();
        _assertGas(vm.lastCallGas(), Gas({gasTotalUsed: 64, gasMemoryUsed: 0, gasRefunded: 0}));

        _performCall();
        _assertGas(vm.lastCallGas(), Gas({gasTotalUsed: 64, gasMemoryUsed: 0, gasRefunded: 0}));

        _performCall();
        _assertGas(vm.lastCallGas(), Gas({gasTotalUsed: 64, gasMemoryUsed: 0, gasRefunded: 0}));
    }

    function testRecordGasRefund() public {
        _setup();
        _performRefund();
        _assertGas(vm.lastCallGas(), Gas({gasTotalUsed: 216, gasMemoryUsed: 0, gasRefunded: 19900}));
        assertEq(vm.snapshotGasLastCall("refund call"), 216);
        assertEq(vm.snapshotGasLastFrame("refund frame"), 216);
    }
}
