// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract ReverterB {
    /// @notice has same error selectors as contract below to test the `reverter` param
    error MyRevert();
    error SpecialRevertWithData(uint256 x);

    function revertIf2(uint256 x) public pure returns (bool) {
        if (x == 2) {
            revert MyRevert();
        }
        return true;
    }

    function revertWithData() public pure returns (bool) {
        revert SpecialRevertWithData(2);
    }
}

contract Reverter {
    error MyRevert();
    error RevertWithData(uint256 x);
    error UnusedError();
    error ExpectedRevertCountZero();

    ReverterB public immutable subReverter;

    constructor() {
        subReverter = new ReverterB();
    }

    function myFunction() public pure returns (bool) {
        revert MyRevert();
    }

    function revertIf2(uint256 value) public pure returns (bool) {
        if (value == 2) {
            revert MyRevert();
        }
        return true;
    }

    function revertWithDataIf2(uint256 value) public pure returns (bool) {
        if (value == 2) {
            revert RevertWithData(2);
        }
        return true;
    }

    function twoPossibleReverts(uint256 x) public pure returns (bool) {
        if (x == 2) {
            revert MyRevert();
        } else if (x == 3) {
            revert RevertWithData(3);
        }
        return true;
    }

    function revertIf2Or3ExpectedRevertZero(uint256 x) public pure returns (bool) {
        if (x == 2) {
            revert ExpectedRevertCountZero();
        } else if (x == 3) {
            revert MyRevert();
        }
        return true;
    }
}

// `test_assume_no_revert_with_data` in `crates/forge/tests/cli/test_cmd.rs`
contract AssumeNoRevertWithDataTest is DSTest {
    Reverter reverter;
    Vm _vm = Vm(HEVM_ADDRESS);

    function setUp() public {
        reverter = new Reverter();
    }

    /// @dev Test that `assumeNoRevert` does not reject an unanticipated error selector
    function testAssume_wrongSelector_fails(uint256 x) public view {
        _vm.assumeNoRevert(Vm.PotentialRevert({revertData: abi.encodeWithSelector(Reverter.UnusedError.selector), partialMatch: false, reverter: address(0)}));
        reverter.revertIf2(x);
    }

    /// @dev Test that `assumeNoRevert` does not reject an unanticipated error with extra data
    function testAssume_wrongData_fails(uint256 x) public view {
        _vm.assumeNoRevert(Vm.PotentialRevert({revertData: abi.encodeWithSelector(Reverter.RevertWithData.selector, 3), partialMatch: false, reverter: address(0)}));
        reverter.revertWithDataIf2(x);
    }

    /// @dev Test that `assumeNoRevert` correctly rejects an error selector from a different contract
    function testAssumeWithReverter_fails(uint256 x) public view {
        ReverterB subReverter = (reverter.subReverter());
        _vm.assumeNoRevert(Vm.PotentialRevert({revertData: abi.encodeWithSelector(Reverter.MyRevert.selector), partialMatch: false, reverter: address(reverter)}));
        subReverter.revertIf2(x);
    }

    /// @dev Test that `assumeNoRevert` correctly rejects one of two different error selectors when supplying a specific reverter
    function testMultipleAssumes_OneWrong_fails(uint256 x) public view {
        Vm.PotentialRevert[] memory revertData = new Vm.PotentialRevert[](2);
        revertData[0] = Vm.PotentialRevert({revertData: abi.encodeWithSelector(Reverter.MyRevert.selector), partialMatch: false, reverter: address(reverter)});
        revertData[1] = Vm.PotentialRevert({revertData: abi.encodeWithSelector(Reverter.RevertWithData.selector, 4), partialMatch: false, reverter: address(reverter)});
        _vm.assumeNoRevert(revertData);
        reverter.twoPossibleReverts(x);
    }

    /// @dev Test that `assumeNoRevert` assumptions are cleared after the first non-cheatcode external call
    function testMultipleAssumesClearAfterCall_fails(uint256 x) public view {
        _vm.assume(x != 3);
        Vm.PotentialRevert[] memory revertData = new Vm.PotentialRevert[](2);
        revertData[0] = Vm.PotentialRevert({revertData: abi.encodeWithSelector(Reverter.MyRevert.selector), partialMatch: false, reverter: address(0)});
        revertData[1] = Vm.PotentialRevert({revertData: abi.encodeWithSelector(Reverter.RevertWithData.selector, 4), partialMatch: false, reverter: address(reverter)});
        _vm.assumeNoRevert(revertData);
        reverter.twoPossibleReverts(x);

        reverter.twoPossibleReverts(2);
    }

    /// @dev Test that `assumeNoRevert` correctly rejects a generic assumeNoRevert call after any specific reason is provided
    function testMultipleAssumes_ThrowOnGenericNoRevert_AfterSpecific_fails(bytes4 selector) public view {
        _vm.assumeNoRevert(Vm.PotentialRevert({revertData: abi.encode(selector), partialMatch: false, reverter: address(0)}));
        _vm.assumeNoRevert();
        reverter.twoPossibleReverts(2);
    }

    function testAssumeThenExpectCountZeroFails(uint256 x) public {
        _vm.assumeNoRevert(
            Vm.PotentialRevert({
                revertData: abi.encodeWithSelector(Reverter.MyRevert.selector),
                partialMatch: false,
                reverter: address(0)
            })
        );
        _vm.expectRevert(Reverter.ExpectedRevertCountZero.selector, 0);
        reverter.revertIf2Or3ExpectedRevertZero(x);
    }

    function testExpectCountZeroThenAssumeFails(uint256 x) public {
        _vm.expectRevert(Reverter.ExpectedRevertCountZero.selector, 0);
        _vm.assumeNoRevert(
            Vm.PotentialRevert({
                revertData: abi.encodeWithSelector(Reverter.MyRevert.selector),
                partialMatch: false,
                reverter: address(0)
            })
        );
        reverter.revertIf2Or3ExpectedRevertZero(x);
    }

}