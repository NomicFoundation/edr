// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

contract InvariantBreaker {
    bool public flag0 = true;
    bool public flag1 = true;

    function set0(int256 val) public returns (bool) {
        if (val % 100 == 0) {
            flag0 = false;
        }
        return flag0;
    }

    function set1(int256 val) public returns (bool) {
        if (val % 10 == 0 && !flag0) {
            flag1 = false;
        }
        return flag1;
    }
}

// Same as `InvariantTest`, but the invariant overrides `runs`/`depth` via inline
// configuration.
contract InvariantOverrideRunsDepth is DSTest {
    InvariantBreaker inv;

    function setUp() public {
        inv = new InvariantBreaker();
    }

    /**
     * forge-config: default.invariant.runs = 1
     * forge-config: default.invariant.depth = 5
     */
    function invariant_neverFalse() public {
        require(inv.flag1(), "false");
    }
}
