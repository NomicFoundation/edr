// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

contract HandlerOne {
    uint256 public hit1;

    function selector1() external {
        hit1 += 1;
    }
}

contract HandlerTwo {
    uint256 public hit2;
    uint256 public hit3;
    uint256 public hit4;
    uint256 public hit5;

    function selector2() external {
        hit2 += 1;
    }

    function selector3() external {
        hit3 += 1;
    }

    function selector4() external {
        hit4 += 1;
    }

    function selector5() external {
        hit5 += 1;
    }
}

contract InvariantSelectorsWeightTest is DSTest {
    HandlerOne handlerOne;
    HandlerTwo handlerTwo;

    function setUp() public {
        handlerOne = new HandlerOne();
        handlerTwo = new HandlerTwo();
    }

    function afterInvariant() public {
        assertEq(handlerOne.hit1(), 2);
        assertEq(handlerTwo.hit2(), 2);
        assertEq(handlerTwo.hit3(), 2);
        assertEq(handlerTwo.hit4(), 1);
        assertEq(handlerTwo.hit5(), 3);
    }

    function invariant_selectors_weight() public view {}
}