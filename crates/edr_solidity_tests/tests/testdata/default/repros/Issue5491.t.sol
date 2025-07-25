// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

// https://github.com/foundry-rs/foundry/issues/5491
contract Issue5491Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testWeirdGas1() public {
        vm.pauseGasMetering();
    }

    function testWeirdGas2() public {
        uint256 a = 1;
        uint256 b = a + 1;
        require(b == 2, "b is not 2");
        vm.pauseGasMetering();
    }

    function testNormalGas() public {
        vm.pauseGasMetering();
        vm.resumeGasMetering();
    }

    function testWithAssembly() public {
        vm.pauseGasMetering();
        assembly {
            return (0, 0)
        }
    }
}
