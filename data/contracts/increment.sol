pragma solidity ^0.8.26;
import "./coverage.sol";

contract Increment {
  uint public x;

  function incBy(uint by) public {
    Coverage.sendHit(abi.encodePacked(uint256(0x0000000000000001)));
    require(by > 0, "Increment should be positive");
    Coverage.sendHit(abi.encodePacked(uint256(0x0000000000000002)));
    x += by;
  }
}
