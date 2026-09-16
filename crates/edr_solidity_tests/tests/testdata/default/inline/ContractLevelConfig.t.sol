// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

contract InlineCounter {
    uint256 public count;

    function increment() public {
        count += 1;
    }
}

// Test functions inherited from a base contract are also covered by the
// derived contract's contract-level configuration.
abstract contract ContractLevelConfigBase is DSTest {
    // Inherited: runs with the derived contract's fuzz.runs = 15.
    function testFuzz_InheritedRuns(uint256 a) public {
        assertEq(a, a);
    }
}

/// forge-config: default.fuzz.runs = 15
/// forge-config: default.invariant.runs = 2
/// forge-config: default.invariant.depth = 3
contract ContractLevelConfigTest is ContractLevelConfigBase {
    InlineCounter counter;

    function setUp() public {
        counter = new InlineCounter();
    }

    // Runs with the contract-level fuzz.runs = 15.
    function testFuzz_ContractLevelRuns(uint256 a) public {
        assertEq(a, a);
    }

    // The function-level directive wins over the contract level.
    /// forge-config: default.fuzz.runs = 20
    function testFuzz_FunctionOverridesContract(uint256 a) public {
        assertEq(a, a);
    }

    // Overloads are distinct tests (distinct selectors); each one runs with
    // the contract-level fuzz.runs = 15.
    function testFuzz_Overloaded(uint256 a) public {
        assertEq(a, a);
    }

    function testFuzz_Overloaded(uint256 a, uint256 b) public {
        assertEq(a, a);
        assertEq(b, b);
    }

    // A function-level directive identifies its function by name only, so it
    // applies to every overload of that name.
    /// forge-config: default.fuzz.runs = 25
    function testFuzz_OverloadedWithDirective(uint256 a) public {
        assertEq(a, a);
    }

    function testFuzz_OverloadedWithDirective(uint256 a, uint256 b) public {
        assertEq(a, a);
        assertEq(b, b);
    }

    // Runs with the contract-level invariant.runs = 2 and depth = 3.
    function invariant_ContractLevelRuns() public {
        assertTrue(address(counter) != address(0));
    }
}
