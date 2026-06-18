// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

// https://github.com/NomicFoundation/edr/issues/1482
//
// When the test sources are compiled with `viaIR`, calling an unsupported
// cheatcode surfaces a cryptic "unrecognized custom error" in the stack trace
// (the `StructuredCheatcodeError` selector `0xdd2ce9c4`) instead of the
// expected "cheatcode '...' is not supported" message. Without `viaIR` the
// same code produces the correct message.
//
// This file is compiled with `via_ir = true` (see the `ViaIr` test profile).
contract Issue1482Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testUnsupportedDeployCode() public {
        // `deployCode(string,bytes)` is unsupported in EDR and reverts with a
        // structured cheatcode error. Use a `bytes` local to select the
        // `(string,bytes)` overload unambiguously.
        bytes memory constructorArgs = hex"";
        vm.deployCode("Issue1482.t.sol", constructorArgs);
    }

    function testUnsupportedBreakpoint() public {
        // `breakpoint(string)` is unsupported in EDR and reverts with a
        // structured cheatcode error.
        vm.breakpoint("a");
    }
}
