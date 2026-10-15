//! From Amsterdam onward (EIP-2780), the flat intrinsic transaction gas is
//! replaced by a base cost plus recipient- and value-based charges.
//! Pre-Amsterdam hardforks keep the flat cost.

use edr_chain_l1::{
    rpc::{call::L1CallRequest, TransactionRequest},
    L1ChainSpec,
};
use edr_primitives::{address, Address, Bytecode, Bytes, U256};
use edr_provider::{
    AccountOverride, MethodInvocation, Provider, ProviderError, ProviderErrorForChainSpec,
    ProviderRequest, ResponseWithCallTraces, TransactionFailureReason,
};
use edr_runtime::transaction::CreationError;
use revm_primitives::{
    eip2780::{TRANSFER_LOG_COST, TX_BASE_COST, TX_VALUE_COST},
    eip8037::{CPSB_GLAMSTERDAM, NEW_ACCOUNT_BYTES},
    eip8038::{COLD_ACCOUNT_ACCESS, CREATE_ACCESS},
};

use crate::common::{
    bytecode::{opcode, BytecodeBuilder},
    provider::{
        estimate_gas, gas_used, new_provider, new_provider_with_config, send_transaction,
        transaction_receipt,
    },
};

const SENDER: Address = address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
/// An account outside the genesis state: a zero-value call leaves it
/// nonexistent, while the first value transfer creates it.
const RECIPIENT: Address = address!("0x70997970C51812dc3A010C7d01b50e0d17dc79C8");
/// A contract whose code needs gas to run.
const CONTRACT_ADDRESS: Address = address!("0x000000000000000000000000000000000000c0de");

/// Flat intrinsic gas of a transaction before Amsterdam.
const PRE_AMSTERDAM_INTRINSIC_GAS: u64 = 21_000;

/// EIP-2780: charge for a nonzero value sent to another account. revm still
/// splits the EIP's single charge into two constants.
const VALUE_CHARGE: u64 = TX_VALUE_COST + TRANSFER_LOG_COST;
/// EIP-8037: state gas of a new account.
const NEW_ACCOUNT_STATE_GAS: u64 = NEW_ACCOUNT_BYTES * CPSB_GLAMSTERDAM;

/// A transfer of `value` from [`SENDER`] to `to`.
fn transfer(to: Address, value: U256) -> TransactionRequest {
    TransactionRequest {
        from: SENDER,
        to: Some(to),
        value: Some(value),
        ..TransactionRequest::default()
    }
}

/// Sends the transaction and returns the `gasUsed` of its receipt.
fn gas_used_by_sending(
    provider: &Provider<L1ChainSpec>,
    request: TransactionRequest,
) -> anyhow::Result<u64> {
    let transaction_hash = send_transaction(provider, request)?;

    Ok(gas_used(provider, transaction_hash))
}

#[tokio::test(flavor = "multi_thread")]
async fn self_transfer_pays_only_the_base_cost_from_amsterdam() -> anyhow::Result<()> {
    let request = transfer(SENDER, U256::from(1));

    let osaka_provider = new_provider(edr_chain_l1::Hardfork::Osaka)?;
    assert_eq!(
        gas_used_by_sending(&osaka_provider, request.clone())?,
        PRE_AMSTERDAM_INTRINSIC_GAS
    );

    // A self-transfer pays neither the recipient nor the value charge.
    let amsterdam_provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;
    assert_eq!(
        gas_used_by_sending(&amsterdam_provider, request)?,
        TX_BASE_COST
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn zero_value_call_pays_base_cost_and_recipient_access_from_amsterdam() -> anyhow::Result<()>
{
    let request = transfer(RECIPIENT, U256::ZERO);

    let osaka_provider = new_provider(edr_chain_l1::Hardfork::Osaka)?;
    assert_eq!(
        gas_used_by_sending(&osaka_provider, request.clone())?,
        PRE_AMSTERDAM_INTRINSIC_GAS
    );

    let amsterdam_provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;
    assert_eq!(
        gas_used_by_sending(&amsterdam_provider, request)?,
        TX_BASE_COST + COLD_ACCOUNT_ACCESS
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn value_transfer_pays_base_cost_recipient_access_and_value_charge_from_amsterdam(
) -> anyhow::Result<()> {
    let request = transfer(RECIPIENT, U256::from(1));

    let osaka_provider = new_provider(edr_chain_l1::Hardfork::Osaka)?;
    // The first transfer creates the recipient; pre-Amsterdam that costs nothing
    // extra.
    assert_eq!(
        gas_used_by_sending(&osaka_provider, request.clone())?,
        PRE_AMSTERDAM_INTRINSIC_GAS
    );
    assert_eq!(
        gas_used_by_sending(&osaka_provider, request.clone())?,
        PRE_AMSTERDAM_INTRINSIC_GAS
    );

    let amsterdam_provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;
    // Creating the recipient is charged as EIP-8037 state gas on top of the
    // intrinsic cost.
    assert_eq!(
        gas_used_by_sending(&amsterdam_provider, request.clone())?,
        TX_BASE_COST + COLD_ACCOUNT_ACCESS + VALUE_CHARGE + NEW_ACCOUNT_STATE_GAS
    );
    // To an existing account, the decomposed charges add up to the flat
    // pre-Amsterdam cost.
    assert_eq!(
        gas_used_by_sending(&amsterdam_provider, request)?,
        TX_BASE_COST + COLD_ACCOUNT_ACCESS + VALUE_CHARGE
    );

    Ok(())
}

// The lower intrinsic cost makes a cheaper transaction valid: funded with
// exactly the intrinsic gas, it is included, and its execution halts out of
// gas as soon as the code runs.
#[tokio::test(flavor = "multi_thread")]
async fn transaction_funding_only_the_intrinsic_gas_is_included_and_halts_from_amsterdam(
) -> anyhow::Result<()> {
    const INTRINSIC_GAS: u64 = TX_BASE_COST + COLD_ACCOUNT_ACCESS;

    /// `PUSH1 1; PUSH1 0; SSTORE; STOP`: a single store is enough to run out of
    /// gas with only the intrinsic gas available.
    fn contract_code() -> Bytes {
        let mut code = BytecodeBuilder::default();
        code.push1(1)
            .push1(0)
            .opcode(opcode::SSTORE)
            .opcode(opcode::STOP);
        code.runtime()
    }

    /// A provider on `hardfork` with [`CONTRACT_ADDRESS`] in its genesis state.
    fn new_provider_with_contract(
        hardfork: edr_chain_l1::Hardfork,
    ) -> anyhow::Result<Provider<L1ChainSpec>> {
        new_provider_with_config(|config| {
            config.hardfork = hardfork;
            // Surface the halt reason through `eth_sendTransaction`.
            config.bail_on_transaction_failure = true;
            config.genesis_state.insert(
                CONTRACT_ADDRESS,
                AccountOverride {
                    code: Some(Bytecode::new_raw(contract_code())),
                    ..AccountOverride::default()
                },
            );
        })
    }

    /// A call to [`CONTRACT_ADDRESS`] with exactly the intrinsic gas.
    fn call_contract_with_intrinsic_gas() -> TransactionRequest {
        TransactionRequest {
            from: SENDER,
            to: Some(CONTRACT_ADDRESS),
            gas: Some(INTRINSIC_GAS),
            ..TransactionRequest::default()
        }
    }

    fn send(
        provider: &Provider<L1ChainSpec>,
        request: TransactionRequest,
    ) -> Result<ResponseWithCallTraces, ProviderErrorForChainSpec<L1ChainSpec>> {
        provider.handle_request(ProviderRequest::with_single(
            MethodInvocation::SendTransaction(request),
        ))
    }

    let request = call_contract_with_intrinsic_gas();

    let osaka_provider = new_provider_with_contract(edr_chain_l1::Hardfork::Osaka)?;
    // Below the flat intrinsic cost, the transaction is not even valid.
    let result = send(&osaka_provider, request.clone());
    assert!(
        matches!(
            result,
            Err(ProviderError::TransactionCreationError(
                CreationError::InsufficientGas {
                    initial_gas_cost: PRE_AMSTERDAM_INTRINSIC_GAS,
                    gas_limit: INTRINSIC_GAS,
                }
            ))
        ),
        "gas limit below the pre-Amsterdam intrinsic gas should be rejected: {result:?}"
    );

    let amsterdam_provider = new_provider_with_contract(edr_chain_l1::Hardfork::Amsterdam)?;
    let result = send(&amsterdam_provider, request);
    let Err(ProviderError::TransactionFailed(failure)) = result else {
        panic!("the call should be mined and reported as failed: {result:?}");
    };
    let failure = &failure.failure;
    assert!(
        matches!(failure.reason, TransactionFailureReason::OutOfGas(_)),
        "the contract's first store should run out of gas: {:?}",
        failure.reason
    );

    let transaction_hash = failure
        .transaction_hash
        .expect("a failed transaction that was mined has a hash");
    let receipt = transaction_receipt(&amsterdam_provider, transaction_hash)?;
    assert_eq!(receipt.status, Some(false));
    // An out-of-gas halt consumes the whole gas limit, which is exactly the
    // intrinsic gas: nothing beyond it was available to the code.
    assert_eq!(receipt.gas_used, INTRINSIC_GAS);

    Ok(())
}

// The estimate must clear the Amsterdam create intrinsic cost, not the flat
// pre-Amsterdam one, and fund the state gas of the new contract account.
#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas_covers_a_contract_creation_from_amsterdam() -> anyhow::Result<()> {
    /// `STOP`: deploys an empty contract.
    fn init_code() -> Bytes {
        let mut code = BytecodeBuilder::default();
        code.opcode(opcode::STOP);
        code.runtime()
    }
    /// One zero calldata byte (`4`) plus one EIP-3860 init code word (`2`).
    const INIT_CODE_INTRINSIC_GAS: u64 = 4 + 2;

    let amsterdam_provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;

    let estimate = estimate_gas(
        &amsterdam_provider,
        L1CallRequest {
            from: Some(SENDER),
            data: Some(init_code()),
            ..L1CallRequest::default()
        },
    );
    assert_eq!(
        estimate,
        TX_BASE_COST + CREATE_ACCESS + INIT_CODE_INTRINSIC_GAS + NEW_ACCOUNT_STATE_GAS
    );

    // Sent with exactly the estimate, the creation goes through.
    let transaction_hash = send_transaction(
        &amsterdam_provider,
        TransactionRequest {
            from: SENDER,
            data: Some(init_code()),
            gas: Some(estimate),
            ..TransactionRequest::default()
        },
    )?;
    let receipt = transaction_receipt(&amsterdam_provider, transaction_hash)?;
    assert_eq!(receipt.status, Some(true), "the creation should succeed");
    assert!(
        receipt.gas_used <= estimate,
        "the estimate ({estimate}) should fund the creation ({})",
        receipt.gas_used
    );

    Ok(())
}
