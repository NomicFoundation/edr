// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

// Test that we report legible error messages for unsupported cheatcodes
contract UnsupportedCheatcodeTest is Test {
    function testUnsupportedCheatcode() public {
        vm.broadcast();
    }
}

contract MissingCheatcodeTest is Test {
    function testMissingCheatcode() public pure {
        vm.getEvmVersion();
    }
}
