// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

contract EnvVarTest is Test {
    function testGetEnv() public view {
        string memory key = "_EDR_SOLIDITY_TESTS_GET_ENV_TEST_KEY";
        string memory val = "_edrSolidityTestsGetEnvTestVal";
        string memory result = vm.envString(key);
        assertEq(result, val);
    }
}
