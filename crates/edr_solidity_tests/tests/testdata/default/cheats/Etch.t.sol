// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract EtchTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testEtch() public {
        // 1-10 are precompiles with the tenth added in Cancun
        address target = address(11);
        bytes memory code = hex"1010";
        vm.etch(target, code);
        assertEq(string(code), string(target.code));
    }

    function testEtchNotAvailableOnPrecompiles() public {
        address target = address(1);
        bytes memory code = hex"1010";
        vm._expectCheatcodeRevert(bytes("cannot call `etch` on precompile 0x0000000000000000000000000000000000000001"));
        vm.etch(target, code);
    }
}
