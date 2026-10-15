// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

// Overloaded fuzz tests must not share one persisted failure file.
contract FuzzOverloadTest is DSTest {
    function testFuzz_overload(address) public pure {
        revert("ADDRESS_BUG");
    }

    function testFuzz_overload(uint256) public pure {
        revert("UINT_BUG");
    }
}
