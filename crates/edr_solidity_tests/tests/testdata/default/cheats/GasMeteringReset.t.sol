// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract B {
    function a() public returns (uint256) {
        return 100;
    }
}

// `gas_metering_reset` in `crates/forge/tests/cli/test_cmd.rs`
contract GasMeteringResetTest is DSTest {
    Vm vm = Vm(HEVM_ADDRESS);
    B b;
    uint256 a;

    function testResetGas() public {
        vm.resetGasMetering();
    }

    function testResetGas1() public {
        vm.resetGasMetering();
        b = new B();
        vm.resetGasMetering();
    }

    function testResetGas2() public {
        b = new B();
        b = new B();
        vm.resetGasMetering();
    }

    function testResetGas3() public {
        vm.resetGasMetering();
        b = new B();
        b = new B();
    }

    function testResetGas4() public {
        vm.resetGasMetering();
        b = new B();
        vm.resetGasMetering();
        b = new B();
    }

    function testResetGas5() public {
        vm.resetGasMetering();
        b = new B();
        vm.resetGasMetering();
        b = new B();
        vm.resetGasMetering();
    }

    function testResetGas6() public {
        vm.resetGasMetering();
        b = new B();
        b = new B();
        _reset();
        vm.resetGasMetering();
    }

    function testResetGas7() public {
        vm.resetGasMetering();
        b = new B();
        b = new B();
        _reset();
    }

    function testResetGas8() public {
        this.resetExternal();
    }

    function testResetGas9() public {
        this.resetExternal();
        vm.resetGasMetering();
    }

    function testResetNegativeGas() public {
        a = 100;
        vm.resetGasMetering();

        delete a;
    }

    function _reset() internal {
        vm.resetGasMetering();
    }

    function resetExternal() external {
        b = new B();
        b = new B();
        vm.resetGasMetering();
    }
}