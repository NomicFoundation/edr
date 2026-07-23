// SPDX-License-Identifier: MIT
pragma solidity ^0.8.34;

/// Base contract in its own file: the build model resolves inherited
/// functions through the base contract's AST (`linearizedBaseContracts`),
/// so this exercises cross-file resolution in a multi-source fixture.
contract ScenarioBase {
    function inheritedFail() public pure {
        require(false, "inherited boom");
    }
}
