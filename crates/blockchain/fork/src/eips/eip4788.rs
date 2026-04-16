use edr_primitives::{address, bytes, Address, Bytecode, Bytes, U256};
use edr_state_api::{account::AccountInfo, StateDiff};

/// The address of the beacon roots contract.
pub const BEACON_ROOTS_ADDRESS: Address = address!("0x000F3df6D732807Ef1319fB7B8bB8522d0Beac02");

/// The bytecode of the beacon roots contract.
pub const BEACON_ROOTS_BYTECODE: Bytes = bytes!(
    "0x3373fffffffffffffffffffffffffffffffffffffffe14604d57602036146024575f5ffd5b5f35801560495762001fff810690815414603c575f5ffd5b62001fff01545f5260205ff35b5f5ffd5b62001fff42064281555f359062001fff015500"
);

/// The history buffer length used by the beacon roots contract.
const HISTORY_BUFFER_LENGTH: u64 = 8191;

/// Storage slot indices for an entry in the EIP-4788 beacon root contract's
/// ring buffer.
///
/// The contract uses two parallel ring buffers to store timestamps and the
/// corresponding parent beacon block roots
///
/// See: <https://eips.ethereum.org/EIPS/eip-4788>
pub struct BeaconRootStorageSlots {
    /// Slot index for the timestamp value.
    pub timestamp_slot: U256,
    /// Slot index for the parent beacon block root
    pub beacon_root_slot: U256,
}

/// Computes the storage slot indices for the given block `timestamp` in the
/// EIP-4788 beacon root contract's ring buffer.
///
/// The contract uses two parallel ring buffers
/// - First buffer to store timestamps.
/// - Second buffer to store the corresponding parent beacon block roots.
pub fn beacon_root_storage_slots(timestamp: u64) -> BeaconRootStorageSlots {
    let timestamp_slot = U256::from(timestamp % HISTORY_BUFFER_LENGTH);
    let beacon_root_slot = timestamp_slot + U256::from(HISTORY_BUFFER_LENGTH);
    BeaconRootStorageSlots {
        timestamp_slot,
        beacon_root_slot,
    }
}

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

#[cfg(test)]
mod tests {
    use edr_primitives::U256;

    use super::*;

    #[test]
    fn beacon_root_storage_slots_follows_eip_spec() {
        let timestamp = HISTORY_BUFFER_LENGTH - 1;
        let slots = beacon_root_storage_slots(timestamp);

        assert_eq!(slots.timestamp_slot, U256::from(timestamp));
        assert_eq!(
            slots.beacon_root_slot,
            U256::from(timestamp + HISTORY_BUFFER_LENGTH)
        );
    }
}
