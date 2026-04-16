pragma solidity ^0.8.26;
import "./coverage.sol";

// TODO: just have the `.sol` file and instrument with tool
contract Increment {
  uint public x;

  function incBy(uint by) public {
    __NomicFoundationCoverage.sendHit(0x0000000000000000000000000000000000000000000000000000000000000001);
    require(by > 0, "Increment should be positive");
    __NomicFoundationCoverage.sendHit(0x0000000000000000000000000000000000000000000000000000000000000002);
    x += by;
  }
}
