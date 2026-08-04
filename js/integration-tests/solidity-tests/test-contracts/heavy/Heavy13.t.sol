// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;
import "../../contracts/heavy/Heavy.sol";
contract Heavy13Test {
  Heavy h;
  function setUp() public { h = new Heavy(); }
  function testFuzzX13(uint256 seed) public { h.work(seed, 250); }
  function testFuzzY13(uint256 seed) public { h.work(seed, 250); }
}
