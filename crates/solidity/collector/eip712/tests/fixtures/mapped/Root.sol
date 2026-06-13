// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@lib/Token.sol";

struct Payment {
    Token token;
    uint256 amount;
}
