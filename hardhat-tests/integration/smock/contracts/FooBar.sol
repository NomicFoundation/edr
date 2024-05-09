// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

contract Foo {
  function f() public {}
}

contract Bar {
  function callFoo(Foo foo) public {
    foo.f();
  }
}
