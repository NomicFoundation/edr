// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

// Repro for https://github.com/NomicFoundation/edr/issues/894
contract C {
    bool public isReverted;

    modifier revertIf() {
        checkReverted();
        _;
    }

    function setIsReverted(bool _isReverted) external {
        isReverted = _isReverted;
    }

    receive() external payable revertIf {}

    function checkReverted() private view {
        require(!isReverted, "I'm reverted");
    }
}
