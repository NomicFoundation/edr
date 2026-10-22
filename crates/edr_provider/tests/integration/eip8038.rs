#![cfg(feature = "test-utils")]

//! EIP-8038: State-Access Gas Cost Update.
//! see <https://eips.ethereum.org/EIPS/eip-8038>
//!
//! From Amsterdam onward, state-access operations are repriced and the
//! storage-clear refund grows. revm prices them; these tests only check that
//! the repricing activates with the hardfork, by comparing costs on Osaka and
//! on Amsterdam.
//!
//! An operation's cost is the `gasUsed` of calling the `StateAccess` function
//! performing it, less the `gasUsed` of calling its `noop`, so the intrinsic
//! gas (also repriced in Amsterdam, by EIP-2780) and the contract's overhead
//! cancel out. The cost's change from Osaka to Amsterdam is then the
//! parameter's change in the EIP's table.

use std::str::FromStr as _;

use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_primitives::{address, keccak256, Address, Bytes, U256};
use edr_provider::{test_utils::deploy_contract, Provider};

use crate::common::provider::{gas_used_by_sending, new_provider};

const STATE_ACCESS_BYTECODE: &str =
    include_str!("../../../../data/deployment_bytecode/StateAccess.bin");

const SENDER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

// Parameters from the EIP's table, before and after Amsterdam.
const PRE_AMSTERDAM_COLD_ACCOUNT_ACCESS: u64 = 2_600;
const COLD_ACCOUNT_ACCESS: u64 = 3_000;
const PRE_AMSTERDAM_STORAGE_CLEAR_REFUND: u64 = 4_800;
const STORAGE_CLEAR_REFUND: u64 = 11_616;

/// The deployed `StateAccess` contract on a provider.
struct StateAccess {
    address: Address,
    provider: Provider<L1ChainSpec>,
}

impl StateAccess {
    fn deploy(hardfork: edr_chain_l1::Hardfork) -> anyhow::Result<Self> {
        let provider = new_provider(hardfork)?;
        let address = deploy_contract(&provider, SENDER, Bytes::from_str(STATE_ACCESS_BYTECODE)?)?;

        Ok(Self { address, provider })
    }

    /// Calls the function with the given signature and arguments and returns
    /// the `gasUsed` of the receipt.
    fn gas_used_by_calling(&self, signature: &str, arguments: &[U256]) -> anyhow::Result<u64> {
        let mut data: Vec<u8> = keccak256(signature).into_iter().take(4).collect();
        for argument in arguments {
            data.extend_from_slice(&argument.to_be_bytes::<32>());
        }

        gas_used_by_sending(
            &self.provider,
            TransactionRequest {
                from: SENDER,
                to: Some(self.address),
                data: Some(data.into()),
                ..TransactionRequest::default()
            },
        )
    }

    /// The gas the function's operation costs on top of a `noop` call.
    fn operation_cost(&self, signature: &str, arguments: &[U256]) -> anyhow::Result<u64> {
        let gas_used = self.gas_used_by_calling(signature, arguments)?;
        let baseline = self.gas_used_by_calling("noop()", &[])?;

        Ok(gas_used - baseline)
    }

    /// The cost of writing `value` to the contract's slot.
    fn write_cost(&self, value: u64) -> anyhow::Result<u64> {
        self.operation_cost("write(uint256)", &[U256::from(value)])
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn cold_account_access_costs_more_from_amsterdam() -> anyhow::Result<()> {
    let osaka = StateAccess::deploy(edr_chain_l1::Hardfork::Osaka)?;
    let osaka_cost = osaka.operation_cost("readBalance()", &[])?;

    let amsterdam = StateAccess::deploy(edr_chain_l1::Hardfork::Amsterdam)?;
    let amsterdam_cost = amsterdam.operation_cost("readBalance()", &[])?;

    assert_eq!(
        amsterdam_cost - osaka_cost,
        COLD_ACCOUNT_ACCESS - PRE_AMSTERDAM_COLD_ACCOUNT_ACCESS,
        "a cold account access should cost more from Amsterdam: {amsterdam_cost} vs {osaka_cost} on Osaka"
    );

    Ok(())
}

// Changing and clearing a nonzero slot cost the same before the refund (up to
// the one calldata byte that differs), so their difference is the refund.
#[tokio::test(flavor = "multi_thread")]
async fn clearing_a_slot_refunds_more_from_amsterdam() -> anyhow::Result<()> {
    fn storage_clear_refund(contract: &StateAccess) -> anyhow::Result<u64> {
        // The slot starts empty; only clearing a slot that was nonzero at the
        // start of the transaction is refunded, so seed it first.
        contract.gas_used_by_calling("write(uint256)", &[U256::from(1)])?;
        let change_cost = contract.write_cost(2)?;
        let clear_cost = contract.write_cost(0)?;

        Ok(change_cost - clear_cost)
    }

    let osaka = StateAccess::deploy(edr_chain_l1::Hardfork::Osaka)?;
    let osaka_refund = storage_clear_refund(&osaka)?;

    let amsterdam = StateAccess::deploy(edr_chain_l1::Hardfork::Amsterdam)?;
    let amsterdam_refund = storage_clear_refund(&amsterdam)?;

    assert_eq!(
        amsterdam_refund - osaka_refund,
        STORAGE_CLEAR_REFUND - PRE_AMSTERDAM_STORAGE_CLEAR_REFUND,
        "clearing a slot should refund more from Amsterdam: {amsterdam_refund} vs {osaka_refund} on Osaka"
    );

    Ok(())
}
