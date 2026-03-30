pragma solidity ^0.8.26;
import "./hardhat_coverage.sol";

contract Increment {
  uint public x;

  function incBy(uint by) public {
    __HardhatCoverage.sendHit(0x0000000000000000000000000000000000000000000000000000000000000001);
    require(by > 0, "Increment should be positive");
    __HardhatCoverage.sendHit(0x0000000000000000000000000000000000000000000000000000000000000002);
    x += by;
  }
}
