// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "./Dep.sol";

struct Mail {
    Person from;
    Person to;
    string contents;
}
