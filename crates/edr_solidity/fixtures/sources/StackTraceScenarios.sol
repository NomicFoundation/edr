// SPDX-License-Identifier: MIT
pragma solidity ^0.8.34;

import "./StackTraceScenariosBase.sol";

/// Stack-trace inference scenarios: plain contracts, one per revert/panic/
/// dispatch shape. Tests pin line numbers here — append, don't shift.

contract NotPayable {
    uint256 public stored;

    function store(uint256 v) public {
        stored = v;
    }
}

contract NoFallback {
    function ping() public pure returns (uint256) {
        return 1;
    }
}

contract NonPayableFallback {
    uint256 public hits;

    fallback() external {
        hits += 1;
    }
}

contract RequiresArgs {
    function needsBoth(uint256 a, uint256 b) public pure returns (uint256) {
        return a + b;
    }
}

interface IReturnsWord {
    function get() external returns (uint256);
}

contract ReturnsNothing {
    function get() external {}
}

contract ExpectsWord {
    function callGet(address target) public returns (uint256) {
        return IReturnsWord(target).get();
    }
}

library ExternalLib {
    function fail() public pure {
        require(false, "external lib boom");
    }
}

contract UsesExternalLib {
    function go() public pure {
        ExternalLib.fail();
    }
}

contract GuardedBareRevert {
    bool public armed = true;

    modifier guarded() {
        if (armed) {
            revert();
        }
        _;
    }

    function fire() external guarded {}
}

contract ValidatedCounter {
    uint256 public count;

    modifier validates(uint256 v) {
        require(v != 13, "unlucky");
        require(v < 1000, "too large");
        _;
        require(count != 666, "post-mortem");
    }

    function bumpIfValid(uint256 v) public validates(v) {
        count = v;
    }
}

contract ValidatedCounterCaller {
    ValidatedCounter public target;

    constructor() {
        target = new ValidatedCounter();
    }

    function callBump(uint256 v) public {
        target.bumpIfValid(v);
    }
}

contract DirectRequire {
    function fail() public pure {
        require(false, "boom");
    }
}

contract AssertFails {
    function fail() public pure {
        assert(false);
    }
}

contract Overflows {
    uint256 public x = type(uint256).max;

    function bump() public {
        x = x + 1;
    }
}

contract DividesByZero {
    function divide() public pure {
        uint256 a = 1;
        uint256 b = 0;
        uint256 c = a / b;
        require(c == c);
    }
}

contract ArrayOutOfBounds {
    function read() public pure {
        uint256[] memory arr = new uint256[](2);
        uint256 v = arr[5];
        require(v == v);
    }
}

contract InvalidEnumCast {
    enum E {
        A,
        B,
        C
    }

    function cast() public pure {
        uint256 raw = 7;
        E e = E(raw);
        require(uint256(e) == uint256(e));
    }
}

contract PopsEmptyArray {
    uint256[] arr;

    function popIt() public {
        arr.pop();
    }
}

contract ThrowsCustomError {
    error MyError(uint256 code, string what);

    function throwIt() public pure {
        revert MyError(42, "custom error");
    }
}

contract ConstructorReverts {
    constructor() {
        require(false, "constructor boom");
    }
}

contract HelperRevertingConstructor {
    function _check(uint256 v) internal pure {
        require(v > 0, "constructor helper boom");
    }

    constructor(uint256 v) {
        _check(v);
    }
}

contract MultipleRequires {
    function check() public pure {
        uint256 x = 1;
        require(x == 1, "first");
        require(x > 1, "second"); // this one fails
    }
}

contract Other {
    function fail() external pure {
        require(false, "called fail");
    }
}

contract CallsOther {
    Other public other;

    constructor() {
        other = new Other();
    }

    function callFail() public view {
        other.fail();
    }
}

contract ModifierGuard {
    modifier onlyPositive(uint256 v) {
        require(v > 0, "modifier must be positive");
        _;
    }

    function setIfPositive(uint256 v) public onlyPositive(v) {}
}

contract DeepRecursion {
    function recurse(uint256 depth) public {
        if (depth == 0) {
            require(false, "bottomed out");
        } else {
            this.recurse(depth - 1);
        }
    }
}

contract InternalRecursion {
    function recurseInternal(uint256 depth) internal pure {
        if (depth == 0) {
            revert("internal bottom");
        } else {
            recurseInternal(depth - 1);
        }
    }

    function start() public pure {
        recurseInternal(3);
    }
}

contract InternalHelperChain {
    uint256 public count;

    function set(uint256 v) public {
        _checkPositive(v);
        count = v;
    }

    function _checkPositive(uint256 v) internal pure {
        require(v > 0, "must be positive");
    }
}

library InternalRevertingLib {
    function alwaysReverts() internal pure {
        require(false, "lib boom");
    }
}

contract UsesInternalLib {
    function go() public pure {
        InternalRevertingLib.alwaysReverts();
    }
}

contract FallbackReverts {
    fallback() external payable {
        revert("fallback boom");
    }
}

contract ReceiveReverts {
    receive() external payable {
        revert("receive boom");
    }
}

contract MutualA {
    MutualB public other;

    function setOther(MutualB b) public {
        other = b;
    }

    function pingA(uint256 d) public {
        if (d == 0) revert("mutual bottom");
        other.pingB(d - 1);
    }
}

contract MutualB {
    MutualA public other;

    function setOther(MutualA a) public {
        other = a;
    }

    function pingB(uint256 d) public {
        other.pingA(d);
    }
}

contract AssemblyReverts {
    function doRevert() public pure {
        assembly {
            mstore(0x00, 0x08c379a000000000000000000000000000000000000000000000000000000000)
            mstore(0x04, 0x20)
            mstore(0x24, 0x05)
            mstore(0x44, 0x61736d6265000000000000000000000000000000000000000000000000000000)
            revert(0x00, 0x64)
        }
    }
}

contract InvalidOpcode {
    function die() public pure {
        assembly {
            invalid()
        }
    }
}

contract InheritsBase is ScenarioBase {
    function localPing() public pure returns (uint256) {
        return 1;
    }
}
