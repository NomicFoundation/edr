// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

// https://github.com/foundry-rs/foundry/issues/5948
contract Issue5948Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testSleepFuzzed(uint256 _milliseconds) public {
        // Limit sleep time to 2 seconds to decrease test time
        uint256 milliseconds = _milliseconds % 2000;

        string[] memory inputs = new string[](2);
        inputs[0] = "date";
        // OS X does not support precision more than 1 second
        inputs[1] = "+%s000";

        bytes memory res = vm.ffi(inputs);
        uint256 start = vm.parseUint(string(res));

        vm.sleep(milliseconds);

        res = vm.ffi(inputs);
        uint256 end = vm.parseUint(string(res));

        // Limit precision to 1000 ms
        assertGe(end - start, milliseconds / 1000 * 1000, "sleep failed");
    }
}
