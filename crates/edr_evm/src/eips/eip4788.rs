use edr_primitives::{address, bytes, Address, Bytecode, Bytes};
use edr_state_api::{account::AccountInfo, StateDiff};

/// The address of the beacon roots contract.
pub const BEACON_ROOTS_ADDRESS: Address = address!("0x000F3df6D732807Ef1319fB7B8bB8522d0Beac02");

/// The bytecode of the beacon roots contract.
pub const BEACON_ROOTS_BYTECODE: Bytes = bytes!(
    "0x3373fffffffffffffffffffffffffffffffffffffffe14604d57602036146024575f5ffd5b5f35801560495762001fff810690815414603c575f5ffd5b62001fff01545f5260205ff35b5f5ffd5b62001fff42064281555f359062001fff015500"
);

pub(crate) fn beacon_roots_contract() -> AccountInfo {
    let code = Bytecode::new_raw(BEACON_ROOTS_BYTECODE);

    AccountInfo {
        nonce: 1,
        code_hash: code.hash_slow(),
        code: Some(code),
        ..AccountInfo::default()
    }
}

pub(crate) fn add_beacon_roots_contract_to_state_diff(state_diff: &mut StateDiff) {
    state_diff.apply_account_change(BEACON_ROOTS_ADDRESS, beacon_roots_contract());
}
