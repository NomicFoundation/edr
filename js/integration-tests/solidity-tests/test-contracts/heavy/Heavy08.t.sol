// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;
import "../../contracts/heavy/Heavy.sol";
contract Heavy08Test {
  Heavy h;
  function setUp() public { h = new Heavy(); }
  function testFuzzX08(uint256 seed) public { h.work(seed, 250); }
  function testFuzzY08(uint256 seed) public { h.work(seed, 250); }
}
