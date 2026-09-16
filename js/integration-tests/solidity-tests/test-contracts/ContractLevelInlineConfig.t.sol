// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import {Test} from "forge-std/src/Test.sol";

// The contract-level directive applies to every test in the contract, with
// function-level directives taking per-key precedence.
/// hardhat-config: default.fuzz.runs = 15
contract ContractLevelInlineConfigTest is Test {
    // Runs with the contract-level fuzz.runs = 15.
    function testFuzz_ContractLevelRuns(uint256 a) public pure {
        assertEq(a, a);
    }

    // The function-level directive wins over the contract level.
    /// hardhat-config: default.fuzz.runs = 20
    function testFuzz_FunctionOverridesContract(uint256 a) public pure {
        assertEq(a, a);
    }
}
