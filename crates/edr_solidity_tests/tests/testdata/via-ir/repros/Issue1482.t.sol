// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

// https://github.com/NomicFoundation/edr/issues/1482
//
// Minimal repro of a test function using unsupported cheatcodes.
contract Issue1482Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testUnsupportedBreakpoint() public {
        // `breakpoint(string)` is unsupported in EDR and reverts with a
        // structured cheatcode error.
        vm.breakpoint("a");
    }
}
