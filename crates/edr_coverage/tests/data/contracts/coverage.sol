pragma solidity ^0.8.26;

library Coverage {
  address constant COVERAGE_ADDRESS =
    0xc0bEc0BEc0BeC0bEC0beC0bEC0bEC0beC0beC0BE;

  function _sendHitImplementation(bytes memory coverageId) private view {
    address coverageAddress = COVERAGE_ADDRESS;
    /// @solidity memory-safe-assembly
    assembly {
      pop(
        staticcall(
          gas(),
          coverageAddress,
          add(coverageId, 32),
          mload(coverageId),
          0,
          0
        )
      )
    }
  }

  function _castToPure(
    function(bytes memory) internal view fnIn
  ) private pure returns (function(bytes memory) pure fnOut) {
    assembly {
      fnOut := fnIn
    }
  }

  function sendHit(bytes memory coverageId) internal pure {
    _castToPure(_sendHitImplementation)(coverageId);
  }
}
