// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";

contract InvariantTargetIncludeTest is Test {
    bool include = true;
    function setUp() public {
       targetContract(address(this));
       bytes4[] memory selectors = new bytes4[](2);
       selectors[0] = this.shouldInclude1.selector;
       selectors[1] = this.shouldInclude2.selector;
       targetSelector(FuzzSelector({addr: address(this), selectors: selectors}));
    }

    function shouldExclude1() public {
        include = false;
    }

    function shouldInclude1() public {
        include = true;
    }

    function shouldExclude2() public {
        include = false;
    }

    function shouldInclude2() public {
        include = true;
    }

    function invariant_include() public view {
        require(include, "does not include");
    }
}

contract InvariantTargetExcludeTest is Test {
    bool include = true;
    function setUp() public {
       targetContract(address(this));
       bytes4[] memory selectors = new bytes4[](2);
       selectors[0] = this.shouldExclude1.selector;
       selectors[1] = this.shouldExclude2.selector;
       excludeSelector(FuzzSelector({addr: address(this), selectors: selectors}));
    }

    function shouldExclude1() public {
        include = false;
    }

    function shouldInclude1() public {
        include = true;
    }

    function shouldExclude2() public {
        include = false;
    }

    function shouldInclude2() public {
        include = true;
    }

    function invariant_exclude() public view {
        require(include, "does not include");
    }
}