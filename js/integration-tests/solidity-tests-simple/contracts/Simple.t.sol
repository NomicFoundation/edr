// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

contract Simple {
  function testThatSucceeds() public pure {
    require(1 == 1, "1 is not equal to 1");
  }

  function testThatFails() public pure {
    require(1 == 2, "1 is not equal to 2");
  }
}
