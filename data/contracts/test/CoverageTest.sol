// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.26;

contract CoverageCall {
    function getValue() public pure returns (uint256) {
        uint256 result = 42;
        return result;
    }

    function willRevert() public pure {
        uint256 x = 1;
        revert("expected revert reason");
    }
}

contract CoverageDeploySuccess {
    uint256 public value;

    constructor() {
        value = 123;
    }
}

contract CoverageDeployRevert {
    constructor() {
        uint256 x = 1;
        revert("constructor failed");
    }
}
