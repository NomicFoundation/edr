// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;
contract Heavy {
  uint256 public acc;
  mapping(uint256 => uint256) public store;
  function work(uint256 seed, uint256 iters) public returns (uint256) {
    uint256 a = seed;
    unchecked { for (uint256 i = 0; i < iters; i++) { a = a * 1103515245 + 12345; store[i % 64] = a; acc = a ^ acc; } }
    return a;
  }
}
