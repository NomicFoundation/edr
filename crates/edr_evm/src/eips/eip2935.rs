use edr_eth::{Address, Bytecode, Bytes, account::AccountInfo, address, bytes};

use crate::state::StateDiff;

/// Address of the history storage contract, introduced in EIP-2935 (Prague
/// hardfork).
pub const HISTORY_STORAGE_ADDRESS: Address = address!("0x0000F90827F1C53a10cb7A02335B175320002935");

/// Bytecode to signal that the history storage contract is unsupported, using a
/// revert reason.
pub const HISTORY_STORAGE_UNSUPPORTED_BYTECODE: Bytes = bytes!(
    "0x60806040526040517f08c379a0000000000000000000000000000000000000000000000000000000008152600401610036906100e8565b60405180910390fd5b600082825260208201905092915050565b7f45495032393335206973206e6f7420737570706f7274656420696e204861726460008201527f686174207965743a2068747470733a2f2f6769746875622e636f6d2f4e6f6d6960208201527f63466f756e646174696f6e2f686172646861742f6973737565732f3632323600604082015250565b60006100d2605f8361003f565b91506100dd82610050565b606082019050919050565b60006020820190508181036000830152610101816100c5565b905091905056fea26469706673582212209faaba1140034f51b16123b009d46640f0f499cf7ca044d2901caa9cf73048dc64736f6c634300081c0033"
);

pub(crate) fn history_storage_contract() -> AccountInfo {
    let code = Bytecode::new_raw(HISTORY_STORAGE_UNSUPPORTED_BYTECODE);

    AccountInfo {
        nonce: 1,
        code_hash: code.hash_slow(),
        code: Some(code),
        ..AccountInfo::default()
    }
}

/// Adds the history storage contract to the provided [`StateDiff`].
pub fn add_history_storage_contract_to_state_diff(state_diff: &mut StateDiff) {
    state_diff.apply_account_change(HISTORY_STORAGE_ADDRESS, history_storage_contract());
}
