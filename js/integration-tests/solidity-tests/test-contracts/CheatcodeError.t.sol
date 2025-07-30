// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

contract Foo {
    function f() public {}
}

contract CheatcodeError is Test {
    function testFunctionDoesntRevertAsExpected() public {
        Foo foo = new Foo();
        vm.expectRevert();
        foo.f();
    }
}
