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
    eip2780::{TX_BASE_COST, TX_VALUE_COST},
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
// The decomposed charges of a value transfer add up to the flat cost they
// replace.
const _: () =
    assert!(TX_BASE_COST + COLD_ACCOUNT_ACCESS + TX_VALUE_COST == PRE_AMSTERDAM_INTRINSIC_GAS);
/// Flat intrinsic gas of a contract creation before Amsterdam: the
/// transaction's plus the creation's.
const PRE_AMSTERDAM_CREATE_INTRINSIC_GAS: u64 = PRE_AMSTERDAM_INTRINSIC_GAS + 32_000;
/// One zero calldata byte (`4`) plus one EIP-3860 init code word (`2`) of
/// [`empty_contract_init_code`].
const INIT_CODE_INTRINSIC_GAS: u64 = 4 + 2;

/// EIP-8037: state gas of a new account.
const NEW_ACCOUNT_STATE_GAS: u64 = NEW_ACCOUNT_BYTES * CPSB_GLAMSTERDAM;

/// A single `STOP`, which returns no runtime code.
fn empty_contract_init_code() -> Bytes {
    let mut code = BytecodeBuilder::default();
    code.opcode(opcode::STOP);
    code.runtime()
}

/// A transfer of `value` from [`SENDER`] to `to`.
fn transfer_request(to: Address, value: U256) -> TransactionRequest {
    TransactionRequest {
        from: SENDER,
        to: Some(to),
        value: Some(value),
        ..TransactionRequest::default()
    }
}

/// A contract creation by [`SENDER`] sending `value` to the new contract.
fn create_request(value: U256) -> TransactionRequest {
    TransactionRequest {
        from: SENDER,
        data: Some(empty_contract_init_code()),
        value: Some(value),
        ..TransactionRequest::default()
    }
}

fn as_call_request(request: &TransactionRequest) -> L1CallRequest {
    L1CallRequest {
        from: Some(request.from),
        to: request.to,
        value: request.value,
        data: request.data.clone(),
        ..L1CallRequest::default()
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
    let request = transfer_request(SENDER, U256::from(1));

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
    let request = transfer_request(RECIPIENT, U256::ZERO);

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
    let request = transfer_request(RECIPIENT, U256::from(1));

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
        TX_BASE_COST + COLD_ACCOUNT_ACCESS + TX_VALUE_COST + NEW_ACCOUNT_STATE_GAS
    );
    // To an existing account, only the decomposed charges are paid.
    assert_eq!(
        gas_used_by_sending(&amsterdam_provider, request)?,
        TX_BASE_COST + COLD_ACCOUNT_ACCESS + TX_VALUE_COST
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn create_pays_base_cost_and_creation_access_regardless_of_value_from_amsterdam(
) -> anyhow::Result<()> {
    let request = create_request(U256::from(1));

    let osaka_provider = new_provider(edr_chain_l1::Hardfork::Osaka)?;
    assert_eq!(
        gas_used_by_sending(&osaka_provider, request.clone())?,
        PRE_AMSTERDAM_CREATE_INTRINSIC_GAS + INIT_CODE_INTRINSIC_GAS
    );

    let amsterdam_provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;
    // The new contract's EIP-8037 state gas is charged on top.
    let with_value = gas_used_by_sending(&amsterdam_provider, request)?;
    assert_eq!(
        with_value,
        TX_BASE_COST + CREATE_ACCESS + INIT_CODE_INTRINSIC_GAS + NEW_ACCOUNT_STATE_GAS
    );
    // With no `TX_VALUE_COST` term, unlike a call with value, a creation
    // without value costs the same.
    assert_eq!(
        gas_used_by_sending(&amsterdam_provider, create_request(U256::ZERO))?,
        with_value
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

#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas_of_a_value_transfer_keeps_the_flat_cost_from_amsterdam() -> anyhow::Result<()>
{
    let request = as_call_request(&transfer_request(RECIPIENT, U256::from(1)));

    let osaka_provider = new_provider(edr_chain_l1::Hardfork::Osaka)?;
    // `eth_estimateGas` never returns less than the intrinsic cost plus one.
    assert_eq!(
        estimate_gas(&osaka_provider, request.clone()),
        PRE_AMSTERDAM_INTRINSIC_GAS + 1
    );

    let amsterdam_provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;
    // Creating the recipient costs state gas, which lifts the estimate above
    // that minimum.
    assert_eq!(
        estimate_gas(&amsterdam_provider, request.clone()),
        PRE_AMSTERDAM_INTRINSIC_GAS + NEW_ACCOUNT_STATE_GAS
    );

    // Once the recipient exists, only the intrinsic gas remains.
    send_transaction(
        &amsterdam_provider,
        transfer_request(RECIPIENT, U256::from(1)),
    )?;
    assert_eq!(
        estimate_gas(&amsterdam_provider, request),
        PRE_AMSTERDAM_INTRINSIC_GAS + 1
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas_of_a_plain_transfer_pays_only_the_reduced_intrinsic_cost_from_amsterdam(
) -> anyhow::Result<()> {
    let osaka_provider = new_provider(edr_chain_l1::Hardfork::Osaka)?;
    let amsterdam_provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;

    // A transfer runs no code, so its estimate is the floor: the intrinsic
    // cost plus one. A self-transfer's is the base cost alone.
    let self_transfer = as_call_request(&transfer_request(SENDER, U256::from(1)));
    assert_eq!(
        estimate_gas(&osaka_provider, self_transfer.clone()),
        PRE_AMSTERDAM_INTRINSIC_GAS + 1
    );
    assert_eq!(
        estimate_gas(&amsterdam_provider, self_transfer),
        TX_BASE_COST + 1
    );

    // The base cost plus the recipient's cold access, without a value charge.
    let zero_value_call = as_call_request(&transfer_request(RECIPIENT, U256::ZERO));
    assert_eq!(
        estimate_gas(&osaka_provider, zero_value_call.clone()),
        PRE_AMSTERDAM_INTRINSIC_GAS + 1
    );
    assert_eq!(
        estimate_gas(&amsterdam_provider, zero_value_call),
        TX_BASE_COST + COLD_ACCOUNT_ACCESS + 1
    );

    Ok(())
}

// The estimate must clear the Amsterdam create intrinsic cost, not the flat
// pre-Amsterdam one, and fund the state gas of the new contract account.
#[tokio::test(flavor = "multi_thread")]
async fn estimate_gas_covers_a_contract_creation_from_amsterdam() -> anyhow::Result<()> {
    let amsterdam_provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;

    let estimate = estimate_gas(
        &amsterdam_provider,
        as_call_request(&create_request(U256::ZERO)),
    );
    assert_eq!(
        estimate,
        TX_BASE_COST + CREATE_ACCESS + INIT_CODE_INTRINSIC_GAS + NEW_ACCOUNT_STATE_GAS
    );

    // Sent with exactly the estimate, the creation goes through.
    let transaction_hash = send_transaction(
        &amsterdam_provider,
        TransactionRequest {
            gas: Some(estimate),
            ..create_request(U256::ZERO)
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
