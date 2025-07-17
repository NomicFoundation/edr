// Note Used in forge-cli tests to assert failures.
// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract Contract {
    function add(uint256 a, uint256 b) public pure returns (uint256) {
        return a + b;
    }
}

contract OtherContract {
    function sub(uint256 a, uint256 b) public pure returns (uint256) {
        return a - b;
    }
}

contract ExpectCreateFailureTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);
    bytes contractBytecode =
        vm.getDeployedCode("ExpectCreateFailures.t.sol:Contract");

    function testFailExpectCreate() public {
        vm.expectCreate(contractBytecode, address(this));
    }

    function testFailExpectCreate2() public {
        vm.expectCreate2(contractBytecode, address(this));
    }

    function testFailExpectCreateWrongBytecode() public {
        vm.expectCreate(contractBytecode, address(this));
        new OtherContract();
    }

    function testFailExpectCreate2WrongBytecode() public {
        vm.expectCreate2(contractBytecode, address(this));
        new OtherContract{salt: "foobar"}();
    }

    function testFailExpectCreateWrongDeployer() public {
        vm.expectCreate(contractBytecode, address(0));
        new Contract();
    }

    function testFailExpectCreate2WrongDeployer() public {
        vm.expectCreate2(contractBytecode, address(0));
        new Contract();
    }

    function testFailExpectCreateWrongScheme() public {
        vm.expectCreate(contractBytecode, address(this));
        new Contract{salt: "foobar"}();
    }

    function testFailExpectCreate2WrongScheme() public {
        vm.expectCreate2(contractBytecode, address(this));
        new Contract();
    }
}
