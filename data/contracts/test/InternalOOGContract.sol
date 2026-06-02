// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract InternalOOGContract {
  uint256 public n;

  function functionToEstimate() public {
    (bool success, ) = address(this).call(
        abi.encodeCall(this.useGas, ())
    );

    success; // Suppress unused variable warning
  }

  function useGas() external {
    // Spend some gas to get to an OOG in the inner call
    for (uint256 i = 0; i < 10000; i++) {
      n += i;
    }
  }
}
