// Auto-generated from data/contracts/test/CoverageTest.sol — do not edit manually.
// Regenerate with:
//   cargo run -p edr_tool_solidity -- --instrument-only data/contracts/test/CoverageTest.sol
// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.26;

/// @dev Validates that coverage instrumentation preserves the returndata buffer
/// across call + returndatacopy and deploy + returndatacopy patterns. Without
/// the fix, coverage probes injected between the .call() and the assembly block
/// would clobber the returndata buffer, causing these functions to return wrong
/// or empty data.
contract CoverageCall {
    Target private target;

    constructor() {
__HardhatCoverage.sendHit(0x2aad45a0f4e6c137c845dcb87d5ccf0050425aeedfd8640527c7572cbc4174bc);         target = new Target();
    }

    /// Forwards a successful call and returns the result via returndatacopy.
    function forwardSuccessfulCall() public returns (uint256) {
__HardhatCoverage.sendHit(0x6ff2899791943d17b402b52972b1aa074563095b94068ad2c5169a5263752294);         (bool success, ) = address(target).call(
            abi.encodeWithSignature("getValue()")
        );
__HardhatCoverage.sendHit(0x256588e9ad34f866f7697b8e06029aaff4e771fbc90bac81949a0857667332ca);         if (!success){__HardhatCoverage.sendHit(0x48574d2e89b4f58f3af38067cd0deb6399773d6a656903c5982f7ce488ea78e6);  {
__HardhatCoverage.sendHit(0x3e1bfa8e3e914b0cbf9dcd11c3e153d98e47049aae3eb641d03af274232a77d1);             revert("call failed");
        }
}__HardhatCoverage.sendHit(0xe46b848b510dcb0f0c22044c7d2591299e1ff8754289c244e6f48239310ceb24);         assembly ("memory-safe") {
            let ptr := mload(0x40)
            returndatacopy(ptr, 0, returndatasize())
            return(ptr, returndatasize())
        }
    }

    /// Forwards a reverted call and returns the original revert data.
    function forwardRevertedCall() public {
__HardhatCoverage.sendHit(0x4500b513ef3732e0f899d016558e2dcb00114d3160187e54cbc6ce5bfd413b84);         (bool success, ) = address(target).call(
            abi.encodeWithSignature("willRevert()")
        );
__HardhatCoverage.sendHit(0x80a0af1b362ffe8309bba622055d0e20b4247dabac0230caa87382b595a3f3fa);         if (success){__HardhatCoverage.sendHit(0x6d675a9bfef9b5b8a385f23726813783682509330ccede6b249b53dffa837973);  {
__HardhatCoverage.sendHit(0x43ee0477d008a153d5247f87116236a5751af93d86c859870958260b242ca07b);             revert("call should have failed");
        }
}__HardhatCoverage.sendHit(0x0e7c43f0a757dc209dbd48d4fe1753a367806e2f46222eda429aef0f3b0e83ce);         assembly ("memory-safe") {
            let ptr := mload(0x40)
            returndatacopy(ptr, 0, returndatasize())
            return(ptr, returndatasize())
        }
    }

    /// Deploys a child contract and returns the returndata after CREATE
    /// (which should be empty on success).
    function deployChild() public {
__HardhatCoverage.sendHit(0x0a9ea854a3f6a215b4a282518fe401945878428aee04da9df0cca26bbe4f84b0);         CoverageDeploySuccess deployed = new CoverageDeploySuccess();
__HardhatCoverage.sendHit(0x418568022208ed5c0454e57f4fe8c2ea638ca802b0c4188c79e3d79f85f3e844);         assembly ("memory-safe") {
            let ptr := mload(0x40)
            returndatacopy(ptr, 0, returndatasize())
            return(ptr, returndatasize())
        }
    }

    /// Deploys a child contract that reverts and returns the original
    /// revert data via returndatacopy.
    function deployRevertingChild() public {
__HardhatCoverage.sendHit(0x268827ced7f009c3c1184510c76f103c4451da1d3343b2eb5e2119de868eb2a6);         try new CoverageDeployRevert() {
__HardhatCoverage.sendHit(0x61a09bf4aeb74601ada4ae71c0185e1c0e8dd8710cd08151140dc0d60fa2513d);             revert("should never happen");
        } catch {
__HardhatCoverage.sendHit(0xc5d35a3731413d7b6e0d550129aebca2d859f7bee5f96d56b918f9ff490c5c3f);             assembly ("memory-safe") {
                let ptr := mload(0x40)
                returndatacopy(ptr, 0, returndatasize())
                return(ptr, returndatasize())
            }
        }
    }
}

contract Target {
    function getValue() external pure returns (uint256) {
__HardhatCoverage.sendHit(0x283400c632b284fc430b9f6290922d47101f807568982116bdf5d1b12dedaf5d);         return 42;
    }

    function willRevert() external pure {
__HardhatCoverage.sendHit(0x9dfc752b9fe417cfae8d12fee61a28c8b655826bba142e518a9e0baa3caf5dd0);         revert("expected revert reason");
    }
}

contract CoverageDeploySuccess {
    uint256 public value;

    constructor() {
__HardhatCoverage.sendHit(0x5ab4509d874f5b2b061be333f5263e4398b3de20593c13484579908169be81ad);         value = 123;
    }
}

contract CoverageDeployRevert {
    constructor() {
__HardhatCoverage.sendHit(0xb045bab740c5afe4a297c440085df58362e32be83711112c9b1e0905bd9d883a);         uint256 x = 1;
__HardhatCoverage.sendHit(0x4c1a1ed151ab63c7ab5271eae7586172495ebb22529fa21953b266edb581a51a);         revert("constructor failed");
    }
}

import "coverage_lib.sol";