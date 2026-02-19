// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

// Adapted from `crates/edr_solidity_tests/tests/testdata/default/cheats/GasSnapshots.t.sol`.
contract GasSnapshotTest is Test {
    uint256 public slot0;
    Flare public flare;

    function setUp() public {
        flare = new Flare();
    }

    function testSnapshotGasSectionExternal() public {
        vm.startSnapshotGas("testAssertGasExternal");
        flare.run(1);
        uint256 gasUsed = vm.stopSnapshotGas();

        assertGt(gasUsed, 0);
    }

    function testSnapshotGasSectionInternal() public {
        vm.startSnapshotGas("testAssertGasInternalA");
        slot0 = 1;
        vm.stopSnapshotGas();

        vm.startSnapshotGas("testAssertGasInternalB");
        slot0 = 2;
        vm.stopSnapshotGas();

        vm.startSnapshotGas("testAssertGasInternalC");
        slot0 = 0;
        vm.stopSnapshotGas();

        vm.startSnapshotGas("testAssertGasInternalD");
        slot0 = 1;
        vm.stopSnapshotGas();

        vm.startSnapshotGas("testAssertGasInternalE");
        slot0 = 2;
        vm.stopSnapshotGas();
    }

    // Writes to `GasSnapshotTest` group with custom names.
    function testSnapshotValueDefaultGroupA() public {
        uint256 a = 123;
        uint256 b = 456;
        uint256 c = 789;

        vm.snapshotValue("a", a);
        vm.snapshotValue("b", b);
        vm.snapshotValue("c", c);
    }

    // Writes to same `GasSnapshotTest` group with custom names.
    function testSnapshotValueDefaultGroupB() public {
        uint256 d = 123;
        uint256 e = 456;
        uint256 f = 789;

        vm.snapshotValue("d", d);
        vm.snapshotValue("e", e);
        vm.snapshotValue("f", f);
    }

    // Writes to `CustomGroup` group with custom names.
    // Asserts that the order of the values is alphabetical.
    function testSnapshotValueCustomGroupA() public {
        uint256 o = 123;
        uint256 i = 456;
        uint256 q = 789;

        vm.snapshotValue("CustomGroup", "q", q);
        vm.snapshotValue("CustomGroup", "i", i);
        vm.snapshotValue("CustomGroup", "o", o);
    }

    // Writes to `CustomGroup` group with custom names.
    // Asserts that the order of the values is alphabetical.
    function testSnapshotValueCustomGroupB() public {
        uint256 x = 123;
        uint256 e = 456;
        uint256 z = 789;

        vm.snapshotValue("CustomGroup", "z", z);
        vm.snapshotValue("CustomGroup", "x", x);
        vm.snapshotValue("CustomGroup", "e", e);
    }

    // Writes to `GasSnapshotTest` group with `testSnapshotGasDefault` name.
    function testSnapshotGasSectionDefaultGroupStop() public {
        vm.startSnapshotGas("testSnapshotGasSection");

        flare.run(256);

        // vm.stopSnapshotGas() will use the last snapshot name.
        uint256 gasUsed = vm.stopSnapshotGas();
        assertGt(gasUsed, 0);
    }

    // Writes to `GasSnapshotTest` group with `testSnapshotGasCustom` name.
    function testSnapshotGasSectionCustomGroupStop() public {
        vm.startSnapshotGas("CustomGroup", "testSnapshotGasSection");

        flare.run(256);

        // vm.stopSnapshotGas() will use the last snapshot name, even with custom group.
        uint256 gasUsed = vm.stopSnapshotGas();
        assertGt(gasUsed, 0);
    }

    // Writes to `GasSnapshotTest` group with `testSnapshotGasSection` name.
    function testSnapshotGasSectionName() public {
        vm.startSnapshotGas("testSnapshotGasSectionName");

        flare.run(256);

        uint256 gasUsed = vm.stopSnapshotGas("testSnapshotGasSectionName");
        assertGt(gasUsed, 0);
    }

    // Writes to `CustomGroup` group with `testSnapshotGasSection` name.
    function testSnapshotGasSectionGroupName() public {
        vm.startSnapshotGas("CustomGroup", "testSnapshotGasSectionGroupName");

        flare.run(256);

        uint256 gasUsed = vm.stopSnapshotGas(
            "CustomGroup",
            "testSnapshotGasSectionGroupName"
        );
        assertGt(gasUsed, 0);
    }

    // Writes to `GasSnapshotTest` group with `testSnapshotGas` name.
    function testSnapshotGasLastCallName() public {
        flare.run(1);

        uint256 gasUsed = vm.snapshotGasLastCall("testSnapshotGasLastCallName");
        assertGt(gasUsed, 0);
    }

    // Writes to `CustomGroup` group with `testSnapshotGas` name.
    function testSnapshotGasLastCallGroupName() public {
        flare.run(1);

        uint256 gasUsed = vm.snapshotGasLastCall(
            "CustomGroup",
            "testSnapshotGasLastCallGroupName"
        );
        assertGt(gasUsed, 0);
    }

    // Calls stopSnapshotGas with a name that doesn't match the startSnapshotGas call.
    function testMismatchedStartStopSnapshot() public {
        vm.startSnapshotGas("testMismatchedStartSnapshot");
        slot0 = 1;
        vm.stopSnapshotGas("testMismatchedStopSnapshot");
    }
}

contract Flare {
    bytes32[] public data;

    function run(uint256 n_) public {
        for (uint256 i = 0; i < n_; i++) {
            data.push(keccak256(abi.encodePacked(i)));
        }
    }
}
