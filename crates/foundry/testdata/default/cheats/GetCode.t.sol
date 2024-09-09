// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract TestContract {}

contract TestContractGetCode {}

contract GetCodeTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testWithPath() public {
        bytes memory code = vm.getCode("default/fork/DssExecLib.sol");
    }

    function testRevert() public {
        vm._expectCheatcodeRevert();
        vm.getCode("ThisContractDoesntExists.sol");
    }

    function testWithVersion() public {
        bytes memory code = vm.getCode("cheats/GetCode.t.sol:TestContract:0.8.18");
        assertEq(type(TestContract).creationCode, code);

        vm._expectCheatcodeRevert("No matching artifact found");
        vm.getCode("cheats/GetCode.t.sol:TestContract:0.8.19");
    }

    function testByName() public {
        bytes memory code = vm.getCode("TestContractGetCode");
        assertEq(type(TestContractGetCode).creationCode, code);
    }

    function testByNameAndVersion() public {
        bytes memory code = vm.getCode("TestContractGetCode:0.8.18");
        assertEq(type(TestContractGetCode).creationCode, code);
    }
}
