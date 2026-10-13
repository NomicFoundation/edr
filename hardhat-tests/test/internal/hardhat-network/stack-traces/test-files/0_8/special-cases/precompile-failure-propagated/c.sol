pragma solidity ^0.8.10;

interface PointEvaluation {
  function verify(bytes calldata input) external view returns (uint256);
}

contract C {
  PointEvaluation internal constant POINT_EVALUATION_PRECOMPILE =
    PointEvaluation(address(0x0a));

  function test() public view {
    POINT_EVALUATION_PRECOMPILE.verify(hex"00");
  }
}
