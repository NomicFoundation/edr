pragma solidity ^0.8.26;
import "../coverage.sol";

// Manually instrumented so the coverageId tags (0x01, 0x02) are visible and
// stable — integration tests assert the received hits against these exact
// values. If you re-instrument this file, make sure the tags still match the test assertions.
// Re-instrument by running edr_tool_solidity with `--instrument-only` flag.
// Recompile with `-i data/contracts/coverage.sol` to resolve the import.
contract Increment {
  uint public x;

  function incBy(uint by) public {
    __NomicFoundationCoverage.sendHit(0x0000000000000000000000000000000000000000000000000000000000000001);
    require(by > 0, "Increment should be positive");
    __NomicFoundationCoverage.sendHit(0x0000000000000000000000000000000000000000000000000000000000000002);
    x += by;
  }
}
