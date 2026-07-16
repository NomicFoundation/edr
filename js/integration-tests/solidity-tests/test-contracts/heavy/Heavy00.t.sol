// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;
import "../../contracts/heavy/Heavy.sol";
contract Heavy00Test {
  Heavy h;
  function setUp() public { h = new Heavy(); }
  function testFuzzX00(uint256 seed) public { h.work(seed, 250); }
  function testFuzzY00(uint256 seed) public { h.work(seed, 250); }
}
