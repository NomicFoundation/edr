// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

// A contract whose `gasleft()` guard demands much more gas than the function
// body actually uses. Gas estimation's initial run (at the full transaction gas
// limit) passes the guard; the re-run at `gas_used` (cheap loop, ~2.1M) fails
// it, triggering the binary search to converge just below the guard threshold.
contract HighGasRequiredContract {
    uint256 public n;

    function functionToEstimate() public {
        require(gasleft() >= 16_700_000, "insufficient remaining gas");

        for (uint256 i = 0; i < 10000; i++) {
            n += 1;
        }
    }
}
