//! EIP-4788: Beacon block root in the EVM
//!
//! This module provides constants and utilities for the EIP-4788 beacon root
//! oracle contract, including the pre-block system call that populates the
//! contract's ring buffer storage.

use edr_primitives::{address, bytes, Address, Bytecode, Bytes, B256, U256};
use edr_state_api::{account::AccountInfo, DynState, EvmStorageSlot, State, StateDiff, StateError};

/// The address of the beacon roots contract.
pub const BEACON_ROOTS_ADDRESS: Address = address!("0x000F3df6D732807Ef1319fB7B8bB8522d0Beac02");

/// The bytecode of the beacon roots contract.
pub const BEACON_ROOTS_BYTECODE: Bytes = bytes!(
    "0x3373fffffffffffffffffffffffffffffffffffffffe14604d57602036146024575f5ffd5b5f35801560495762001fff810690815414603c575f5ffd5b62001fff01545f5260205ff35b5f5ffd5b62001fff42064281555f359062001fff015500"
);

/// The history buffer length used by the beacon roots contract (0x1fff).
const HISTORY_BUFFER_LENGTH: u64 = 8191;

/// Genesis parent beacon block root if EIP-4788 is active at genesis
pub const GENESIS_PARENT_BEACON_BLOCK_ROOT: B256 = B256::ZERO;

/// Creates the [`AccountInfo`] for the beacon roots contract.
pub fn beacon_roots_contract() -> AccountInfo {
    let code = Bytecode::new_raw(BEACON_ROOTS_BYTECODE);

    AccountInfo {
        nonce: 1,
        code_hash: code.hash_slow(),
        code: Some(code),
        ..AccountInfo::default()
    }
}

/// Adds the beacon roots contract to the provided state diff.
pub fn add_beacon_roots_contract_to_state_diff(state_diff: &mut StateDiff) {
    state_diff.apply_account_change(BEACON_ROOTS_ADDRESS, beacon_roots_contract());
}

/// Applies the EIP-4788 beacon root system call by writing the
/// `parent_beacon_block_root` into the beacon roots contract's ring buffer
/// storage.
///
/// Per EIP-4788, at the start of each block (Cancun+), the system writes:
/// - Slot `timestamp % 8191` = `timestamp`
/// - Slot `(timestamp % 8191) + 8191` = `parent_beacon_block_root`
pub fn apply_beacon_root_contract_call(
    state: &mut dyn DynState,
    state_diff: &mut StateDiff,
    timestamp: u64,
    parent_beacon_block_root: B256,
) -> Result<(), StateError> {
    // If this EIP is active in a genesis block, the genesis header’s
    // parent_beacon_block_root must be 0x0 and no system transaction may occur.
    // See: https://eips.ethereum.org/EIPS/eip-4788
    if parent_beacon_block_root == GENESIS_PARENT_BEACON_BLOCK_ROOT {
        return Ok(());
    }

    // Only execute the system call if the beacon root contract is deployed.
    // In local mode the contract is injected via genesis state by the caller.
    // If it’s absent, skip silently.
    let Some(account_info) = state.basic(BEACON_ROOTS_ADDRESS)? else {
        return Ok(());
    };
    state_diff.apply_account_change(BEACON_ROOTS_ADDRESS, account_info);

    let timestamp_index = U256::from(timestamp % HISTORY_BUFFER_LENGTH);
    let root_index = timestamp_index + U256::from(HISTORY_BUFFER_LENGTH);

    let timestamp_value = U256::from(timestamp);
    let root_value = U256::from_be_bytes(parent_beacon_block_root.0);

    let old_timestamp =
        state.set_account_storage_slot(BEACON_ROOTS_ADDRESS, timestamp_index, timestamp_value)?;
    state_diff.apply_storage_change(
        BEACON_ROOTS_ADDRESS,
        timestamp_index,
        EvmStorageSlot::new_changed(old_timestamp, timestamp_value, 0),
        None,
    );

    let old_root = state.set_account_storage_slot(BEACON_ROOTS_ADDRESS, root_index, root_value)?;
    state_diff.apply_storage_change(
        BEACON_ROOTS_ADDRESS,
        root_index,
        EvmStorageSlot::new_changed(old_root, root_value, 0),
        None,
    );

    Ok(())
}
