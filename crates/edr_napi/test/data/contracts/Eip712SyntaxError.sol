// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

// Deliberately broken: backs the N-API test asserting that a test source
// which cannot be parsed to an AST is skipped with a warning rather than
// failing the run. There is no
// compiled artifact for this file; the test points an existing suite's
// `testSourcePaths` entry at it, simulating an on-disk source that diverged
// from its compiled artifact.
contract Eip712SyntaxError {
    function testBroken() public { this is not solidity
}
