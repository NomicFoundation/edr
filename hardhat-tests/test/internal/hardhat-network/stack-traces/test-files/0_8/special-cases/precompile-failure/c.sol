pragma solidity ^0.8.1;

contract C {
  address internal constant POINT_EVALUATION_PRECOMPILE_ADDRESS = address(0x0a);

  function test() public {
    bytes memory forcingFailureBytes = "0x000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

    _verifyPointEvaluation(0x0, 0x0, 0x0, forcingFailureBytes, forcingFailureBytes);
  }

  function _verifyPointEvaluation(
    bytes32 _currentDataHash,
    uint256 _dataEvaluationPoint,
    uint256 _dataEvaluationClaim,
    bytes memory _kzgCommitment,
    bytes memory _kzgProof
  ) internal view {
    POINT_EVALUATION_PRECOMPILE_ADDRESS.staticcall(
      abi.encodePacked(_currentDataHash, _dataEvaluationPoint, _dataEvaluationClaim, _kzgCommitment, _kzgProof)
    );
  }
}
