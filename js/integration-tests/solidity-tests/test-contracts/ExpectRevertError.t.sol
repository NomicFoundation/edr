// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

contract Foo {
    function f() public {
    }

    function g() public {
        revert("revert with a different message");
    }
}

contract ExpectRevertErrorTest is Test {
    function testFunctionDoesntRevertAsExpected() public {
        Foo foo = new Foo();
        vm.expectRevert();
        foo.f();
    }

    function testFunctionRevertsWithDifferentMessage() public {
        Foo foo = new Foo();
        vm.expectRevert("expected message");
        foo.g();
    }

    function testFunctionRevertCountMismatch() public {
        Foo foo = new Foo();
        vm.expectRevert(2);
        foo.g();
        foo.f();
    }
}
