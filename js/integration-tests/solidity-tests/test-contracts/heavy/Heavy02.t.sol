// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;
import "../../contracts/heavy/Heavy.sol";
contract Heavy02Test {
  Heavy h;
  function setUp() public { h = new Heavy(); }
  function testFuzzX02(uint256 seed) public { h.work(seed, 250); }
  function testFuzzY02(uint256 seed) public { h.work(seed, 250); }
}
