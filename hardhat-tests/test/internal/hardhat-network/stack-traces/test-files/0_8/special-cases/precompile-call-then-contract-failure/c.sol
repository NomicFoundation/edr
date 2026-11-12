pragma solidity ^0.8.0;

import "./d.sol";

contract C {
  address internal constant IDENTITY_PRECOMPILE = address(0x04);

  function test() public {
    D d = new D();
    (bool ok, ) = IDENTITY_PRECOMPILE.staticcall(hex"01");
    require(ok, "identity failed");
    d.fail();
  }
}
