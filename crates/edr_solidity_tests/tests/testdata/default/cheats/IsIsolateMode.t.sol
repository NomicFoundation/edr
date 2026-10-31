// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract IsIsolateModeTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testIsolateModeDisabled() public {
        assertTrue(!vm.isIsolateMode());
    }

    /// forge-config: default.isolate = true
    function testIsolateModeEnabledByOverride() public {
        assertTrue(vm.isIsolateMode());
    }
}

/// Runs in the isolated cheatcode suite.
contract IsIsolateModeIsolated is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testIsolateModeEnabled() public {
        assertTrue(vm.isIsolateMode());
    }

    /// forge-config: default.isolate = false
    function testIsolateModeDisabledByOverride() public {
        assertTrue(!vm.isIsolateMode());
    }
}
