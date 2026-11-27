// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

contract EvmVersionAmsterdamTest is DSTest {
    function testCall() public {
        assertEq(uint256(1) + 1, 2);
    }
}

/// forge-config: default.evm_version = "Osaka"
contract EvmVersionOsakaTest is DSTest {
    function testCall() public {
        assertEq(uint256(1) + 1, 2);
    }
}
