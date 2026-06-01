// SPDX-License-Identifier: MIT
pragma solidity ^0.8.34;

import "forge-std/Test.sol";

/// Tier-1 scenarios: each test triggers a different revert/panic class, and
/// expects EDR to render a Solidity stack trace. The driver script asserts
/// solx and solc produce equivalent traces for every test in this file.

contract DirectRequireTest is Test {
  function testDirectRequire() public pure {
    require(false, "boom");
  }
}

contract AssertionFailureTest is Test {
  function testAssertionFails() public pure {
    assert(false);
  }
}

contract OverflowTest is Test {
  uint256 public x = type(uint256).max;

  function testOverflow() public {
    x = x + 1;
  }
}

contract DivisionByZeroTest is Test {
  function testDivisionByZero() public pure {
    uint256 a = 1;
    uint256 b = 0;
    uint256 c = a / b;
    require(c == c);
  }
}

contract ArrayOutOfBoundsTest is Test {
  function testArrayOOB() public pure {
    uint256[] memory arr = new uint256[](2);
    uint256 v = arr[5];
    require(v == v);
  }
}

contract CustomErrorTest is Test {
  error MyError(uint256 code, string what);

  function testCustomError() public pure {
    revert MyError(42, "custom error");
  }
}

contract ConstructorRevertContract {
  constructor() {
    require(false, "constructor boom");
  }
}

contract ConstructorRevertTest is Test {
  function testConstructorRevert() public {
    new ConstructorRevertContract();
  }
}

contract Other {
  function fail() external pure {
    require(false, "called fail");
  }
}

contract CrossContractCallTest is Test {
  Other other;

  function setUp() public {
    other = new Other();
  }

  function testCrossContractCall() public view {
    other.fail();
  }
}

contract ModifierTarget {
  modifier onlyPositive(uint256 v) {
    require(v > 0, "modifier must be positive");
    _;
  }

  function setIfPositive(uint256 v) public onlyPositive(v) {}
}

contract ModifierRevertTest is Test {
  ModifierTarget t;

  function setUp() public {
    t = new ModifierTarget();
  }

  function testModifierRevert() public {
    t.setIfPositive(0);
  }
}

contract DeepRecursionTarget {
  function recurse(uint256 depth) public {
    if (depth == 0) {
      require(false, "bottomed out");
    } else {
      this.recurse(depth - 1);
    }
  }
}

contract DeepRecursionTest is Test {
  DeepRecursionTarget t;

  function setUp() public {
    t = new DeepRecursionTarget();
  }

  function testDeepRecursion() public {
    t.recurse(3);
  }
}

contract InlineAssemblyRevertTest is Test {
  function testInlineAssemblyRevert() public pure {
    assembly {
      mstore(0x00, 0x08c379a000000000000000000000000000000000000000000000000000000000)
      mstore(0x04, 0x20)
      mstore(0x24, 0x05)
      mstore(0x44, 0x61736d6265000000000000000000000000000000000000000000000000000000)
      revert(0x00, 0x64)
    }
  }
}

contract InternalHelperChainContract {
  uint256 public count;

  function set(uint256 v) public {
    _checkPositive(v);
    count = v;
  }

  function _checkPositive(uint256 v) internal pure {
    require(v > 0, "must be positive");
  }
}

contract InternalHelperChainTest is Test {
  InternalHelperChainContract c;

  function setUp() public {
    c = new InternalHelperChainContract();
  }

  function testInternalHelperChain() public {
    c.set(0);
  }
}

contract InvalidEnumCastTest is Test {
  enum E { A, B, C }
  function testInvalidEnumCast() public pure {
    uint256 raw = 7;
    E e = E(raw);
    require(uint256(e) == uint256(e));
  }
}

contract PopEmptyArrayTest is Test {
  uint256[] arr;
  function testPopEmpty() public {
    arr.pop();
  }
}

contract InvalidOpcodeTest is Test {
  function testInvalidOpcode() public pure {
    assembly { invalid() }
  }
}

// ===== vm.expectRevert cheatcode-violation traces =====

contract ExpectRevertNoActualRevertTest is Test {
  function testNoActualRevert() public {
    vm.expectRevert();
    // Function below does NOT revert; cheatcode should fire its own error.
    uint256 x = 1 + 1;
    x;
  }
}

contract ExpectRevertWrongMessageTest is Test {
  function inner() external pure { revert("actual"); }
  function testWrongMessage() public {
    vm.expectRevert(bytes("expected"));
    this.inner();
  }
}

contract ExpectRevertCountMismatchTest is Test {
  function testCountMismatch() public {
    vm.expectRevert(bytes("boom"));
    vm.expectRevert(bytes("boom"));
    revert("boom"); // satisfies only the most recent expectRevert; the queued earlier one is unmet.
  }
}

// ===== fuzz failures =====

contract OverflowFuzzTest is Test {
  function testFuzz_overflow(uint256 x) public pure {
    uint256 _max = type(uint256).max;
    require(x <= _max, "always true placeholder"); // forces fuzzing input range
    uint256 y = x + 1;
    y;
  }
}

// ===== trace-shape categories =====

library RevertingLib {
  function alwaysReverts() internal pure {
    require(false, "lib boom");
  }
}

contract LibraryRevertTest is Test {
  using RevertingLib for *;
  function testLibraryRevert() public pure {
    RevertingLib.alwaysReverts();
  }
}

contract DelegatecallTargetReverts {
  function doFail() external pure {
    require(false, "delegate boom");
  }
}

contract DelegatecallRevertTest is Test {
  DelegatecallTargetReverts t;
  function setUp() public {
    t = new DelegatecallTargetReverts();
  }
  function testDelegatecallRevert() public {
    (bool ok, ) = address(t).delegatecall(abi.encodeWithSelector(DelegatecallTargetReverts.doFail.selector));
    require(ok, "delegatecall failed");
  }
}

contract FallbackRevertTarget {
  fallback() external payable {
    revert("fallback boom");
  }
}

contract FallbackRevertTest is Test {
  FallbackRevertTarget t;
  function setUp() public {
    t = new FallbackRevertTarget();
  }
  function testFallbackRevert() public {
    (bool ok, ) = address(t).call(abi.encodeWithSelector(bytes4(keccak256("nonExistent()"))));
    require(ok, "fallback didn't revert?");
  }
}

contract ReceiveRevertTarget {
  receive() external payable {
    revert("receive boom");
  }
}

contract ReceiveRevertTest is Test {
  ReceiveRevertTarget t;
  function setUp() public {
    t = new ReceiveRevertTarget();
  }
  function testReceiveRevert() public {
    (bool ok, ) = address(t).call{value: 0}("");
    require(ok, "receive didn't revert?");
  }
}

// ===== additional constructor revert (with internal helper) =====

contract HelperRevertingConstructorContract {
  function _check(uint256 v) internal pure {
    require(v > 0, "constructor helper boom");
  }
  constructor(uint256 v) {
    _check(v);
  }
}

contract HelperRevertingConstructorTest is Test {
  function testHelperRevertingConstructor() public {
    new HelperRevertingConstructorContract(0);
  }
}

// ===== additional custom error variants =====

contract CustomErrorWithArgsTest is Test {
  error InvalidArg(uint256 got, string what);
  function testCustomErrorWithArgs() public pure {
    revert InvalidArg(42, "out of range");
  }
}

contract CustomErrorRecursiveTest is Test {
  error Boom();
  function inner() internal pure { revert Boom(); }
  function testCustomErrorViaInternal() public pure {
    inner();
  }
}

// ===== additional require / assertion failures =====

contract NestedRequireTest is Test {
  function check(uint256 v) internal pure {
    require(v > 0, "nested check failed");
  }
  function testNestedRequire() public pure {
    check(0);
  }
}

contract MultipleRequiresTest is Test {
  function testMultipleRequires() public pure {
    uint256 x = 1;
    require(x == 1, "first");
    require(x > 1, "second"); // this one fails
  }
}

// ===== internal recursion preserved in inline_call_sites =====
contract InternalRecurseTest is Test {
  function recurseInternal(uint256 depth) internal pure {
    if (depth == 0) {
      revert("internal bottom");
    } else {
      recurseInternal(depth - 1);
    }
  }
  function testInternalRecurse() public pure {
    recurseInternal(3);
  }
}

// ===== invariant test failure =====
contract InvariantFailureTest is Test {
  function invariant_alwaysFalse() public pure {
    require(false, "invariant boom");
  }
}

// ===== cross-CALL mutual recursion across contracts =====
//
// Two contracts whose external functions call each other recursively
// across CALL boundaries. solx may emit `JumpType::IntoFunction` JUMPs
// at the dispatch point AND inlined-subroutine entries for some of the
// same call sites — exercises whether `build_solx_inline_callstack_frames`
// double-stacks frames against the JUMP-derived ones already pushed by
// `raw_trace_evm_execution`.
contract MutualA {
  MutualB other;
  function setOther(MutualB b) public { other = b; }
  function pingA(uint256 d) public {
    if (d == 0) revert("mutual bottom");
    other.pingB(d - 1);
  }
}

contract MutualB {
  MutualA other;
  function setOther(MutualA a) public { other = a; }
  function pingB(uint256 d) public {
    other.pingA(d);
  }
}

contract MutualRecursionTest is Test {
  MutualA a;
  MutualB b;
  function setUp() public {
    b = new MutualB();
    a = new MutualA();
    a.setOther(b);
    b.setOther(a);
  }
  function testMutualRecursion() public {
    a.pingA(2);
  }
}

// ===== modifier with statements that may keep it as its own subroutine =====
//
// A modifier with multiple `require`s before and after the underscore.
// solx's optimizer may flatten this into the modified function (as it
// does for the simpler `ModifierTarget`) OR keep it as its own
// `DW_TAG_inlined_subroutine`. If the latter, `build_solx_inline_callstack_frames`
// classifies the bottom frame as `Modifier` — exercises whether the
// modifier-handler path doubles up the wrapping function's frame.
contract NestedModifierTarget {
  uint256 public count;
  modifier validates(uint256 v) {
    require(v != 13, "unlucky");
    require(v < 1000, "too large");
    _;
    require(count != 666, "post-mortem");
  }
  function bumpIfValid(uint256 v) public validates(v) { count = v; }
}

contract NestedModifierRevertTest is Test {
  NestedModifierTarget t;
  function setUp() public { t = new NestedModifierTarget(); }
  function testRevertInModifierBody() public {
    // Hits the `unlucky` require in the modifier's pre-`_` body.
    t.bumpIfValid(13);
  }
}
