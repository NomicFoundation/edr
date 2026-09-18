// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

// Profiles are resolved separately at the contract level and at each function,
// and only then is the contract's result applied underneath the function's. So
// a function-level directive wins over a contract-level one regardless of
// which of the two carries a profile prefix.
/// forge-config: fuzz.runs = 15
/// forge-config: ci.fuzz.runs = 8
contract ProfileLevelPrecedenceTest is DSTest {
    // No directive of its own: takes the contract's value under the selected
    // profile (15 under `default`, 8 under `ci`).
    function testFuzz_ContractLevelOnly(uint256 a) public {
        assertEq(a, a);
    }

    // The function's unprefixed directive beats the contract's `ci.` one even
    // under `ci`: level precedence outranks profile precedence.
    /// forge-config: fuzz.runs = 20
    function testFuzz_FunctionBareBeatsContractProfile(uint256 a) public {
        assertEq(a, a);
    }

    // Under `ci` the function's prefixed directive wins. Under `default` it is
    // inert, so the function has no configuration of its own and falls back to
    // the contract's unprefixed value.
    /// forge-config: ci.fuzz.runs = 30
    function testFuzz_FunctionProfileFallsBackToContract(uint256 a) public {
        assertEq(a, a);
    }
}

// A contract whose only directive targets an unselected profile contributes
// nothing, leaving its tests on the global configuration.
/// forge-config: ci.fuzz.runs = 8
contract ProfileOnlyContractLevelTest is DSTest {
    function testFuzz_NoDirective(uint256 a) public {
        assertEq(a, a);
    }
}
