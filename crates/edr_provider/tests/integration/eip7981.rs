#![cfg(feature = "test-utils")]

//! EIP-7981: Increase access list cost.
//! see <https://eips.ethereum.org/EIPS/eip-7981>
//
//! From Amsterdam onward, access-list data is charged at 64 gas per byte as a
//! flat surcharge on top of both the intrinsic gas and the EIP-7623 gas floor,
//! so access lists can no longer bypass the floor pricing.

use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_eip2930::AccessListItem;
use edr_primitives::{address, Address, Bytes, B256};
use edr_provider::Provider;

use crate::common::provider::{gas_used, new_provider, send_transaction};

const SENDER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

/// Sends the transaction and asserts the `gasUsed` reported by its receipt.
fn assert_transaction_gas_usage(
    provider: &Provider<L1ChainSpec>,
    request: TransactionRequest,
    expected_gas_usage: u64,
) {
    let transaction_hash = send_transaction(provider, request).expect("transaction should succeed");

    let gas_used = gas_used(provider, transaction_hash);
    assert_eq!(gas_used, expected_gas_usage);
}

const TX_BASE_COST: u64 = 21_000;

/// EIP-2930 per-item access list costs, unchanged by EIP-7981.
const ACCESS_LIST_ADDRESS_COST: u64 = 2_400;
const ACCESS_LIST_STORAGE_KEY_COST: u64 = 1_900;

/// Intrinsic cost of a nonzero calldata byte (EIP-2028).
const NONZERO_CALLDATA_BYTE_COST: u64 = 16;

/// EIP-7623 floor cost of a nonzero calldata byte (4 tokens at 10 gas each).
const EIP7623_FLOOR_COST_PER_NONZERO_BYTE: u64 = 40;

/// EIP-7981 surcharge: 64 gas per byte of access-list data, for the single
/// entry's 20-byte address and 32-byte storage key.
const ACCESS_LIST_DATA_COST: u64 = 64 * (20 + 32);

/// EIP-7976 floor cost of a calldata byte (4 tokens at 16 gas each).
const EIP7976_CALLDATA_FLOOR_COST_PER_BYTE: u64 = 64;

const CALLDATA_BYTES: u64 = 100;

/// An access list with one address (20 bytes) and one storage key (32 bytes).
fn access_list() -> Vec<AccessListItem> {
    vec![AccessListItem {
        address: address!("0x70997970C51812dc3A010C7d01b50e0d17dc79C8"),
        storage_keys: vec![B256::ZERO],
    }]
}

/// Intrinsic gas of [`access_list_transfer`]: base cost plus the EIP-2930
/// per-item charges, unchanged by EIP-7981.
const ACCESS_LIST_TRANSFER_INTRINSIC_GAS: u64 =
    TX_BASE_COST + ACCESS_LIST_ADDRESS_COST + ACCESS_LIST_STORAGE_KEY_COST;

/// Intrinsic gas of [`access_list_and_calldata_transfer`], adding the calldata
/// cost. Also unchanged by EIP-7981.
const ACCESS_LIST_AND_CALLDATA_TRANSFER_INTRINSIC_GAS: u64 =
    ACCESS_LIST_TRANSFER_INTRINSIC_GAS + CALLDATA_BYTES * NONZERO_CALLDATA_BYTE_COST;

/// A transfer to an EOA carrying only an access list, so its gas usage is the
/// intrinsic cost including the per-item access list charges.
fn access_list_transfer() -> TransactionRequest {
    TransactionRequest {
        from: SENDER,
        to: Some(SENDER),
        access_list: Some(access_list()),
        ..TransactionRequest::default()
    }
}

/// The same transfer carrying [`CALLDATA_BYTES`] nonzero calldata bytes,
/// making the gas floor the binding cost from Amsterdam onward.
fn access_list_and_calldata_transfer() -> TransactionRequest {
    TransactionRequest {
        data: Some(Bytes::from(vec![0x11; CALLDATA_BYTES as usize])),
        ..access_list_transfer()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn access_list_item_costs_increase_in_amsterdam() -> anyhow::Result<()> {
    let osaka_expected_gas_used = ACCESS_LIST_TRANSFER_INTRINSIC_GAS;
    let osaka_provider = new_provider(edr_chain_l1::Hardfork::Osaka)?;
    assert_transaction_gas_usage(
        &osaka_provider,
        access_list_transfer(),
        osaka_expected_gas_used,
    );

    let amsterdam_expected_gas_used = osaka_expected_gas_used + ACCESS_LIST_DATA_COST;
    let amsterdam_provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;
    assert_transaction_gas_usage(
        &amsterdam_provider,
        access_list_transfer(),
        amsterdam_expected_gas_used,
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn access_list_bytes_count_toward_gas_floor_in_amsterdam() -> anyhow::Result<()> {
    // The intrinsic gas and the floor are unchanged by EIP-7981; the data
    // surcharge is added on top of whichever the transaction pays.
    let intrinsic_gas = ACCESS_LIST_AND_CALLDATA_TRANSFER_INTRINSIC_GAS;

    let osaka_floor_gas = TX_BASE_COST + CALLDATA_BYTES * EIP7623_FLOOR_COST_PER_NONZERO_BYTE;
    assert!(
        intrinsic_gas > osaka_floor_gas,
        "fixture must exceed the EIP-7623 floor before Amsterdam, so the intrinsic gas is used"
    );

    let osaka_provider = new_provider(edr_chain_l1::Hardfork::Osaka)?;
    assert_transaction_gas_usage(
        &osaka_provider,
        access_list_and_calldata_transfer(),
        intrinsic_gas,
    );

    let amsterdam_floor_gas = TX_BASE_COST + CALLDATA_BYTES * EIP7976_CALLDATA_FLOOR_COST_PER_BYTE;
    assert!(
        amsterdam_floor_gas > intrinsic_gas,
        "fixture must stay below the EIP-7976 floor from Amsterdam, so the floor is paid"
    );

    let amsterdam_provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;
    assert_transaction_gas_usage(
        &amsterdam_provider,
        access_list_and_calldata_transfer(),
        amsterdam_floor_gas + ACCESS_LIST_DATA_COST,
    );

    Ok(())
}
