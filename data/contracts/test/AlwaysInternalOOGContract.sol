// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

// Variant of `InternalOOGContract` whose inner call OOGs even at the block
// gas limit, so the OOG-aware estimation has no valid value to converge to
// and must fall back to the initial estimation. The outer call catches the
// failure as in the normal contract.
contract AlwaysInternalOOGContract {
  uint256 public n;

  function functionToEstimate() public {
    (bool success, ) = address(this).call(
        abi.encodeCall(this.useGas, ())
    );

    success; // Suppress unused variable warning
  }

  function useGas() external {
    // Spend gas in an unbounded loop so the inner sub-call always runs out
    // of gas, no matter what gas limit the outer transaction provides.
    while (true) {
      n += 1;
    }
  }
}
