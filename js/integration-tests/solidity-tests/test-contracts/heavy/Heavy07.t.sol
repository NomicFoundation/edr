// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;
import "../../contracts/heavy/Heavy.sol";
contract Heavy07Test {
  Heavy h;
  function setUp() public { h = new Heavy(); }
  function testFuzzX07(uint256 seed) public { h.work(seed, 250); }
  function testFuzzY07(uint256 seed) public { h.work(seed, 250); }
}
