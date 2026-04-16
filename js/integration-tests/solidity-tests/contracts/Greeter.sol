// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

contract Greeter {
    string public greeting;

    constructor(string memory _greeting) {
        greeting = _greeting;
    }
}
