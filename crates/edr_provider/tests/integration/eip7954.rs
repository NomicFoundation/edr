#![cfg(feature = "test-utils")]

//! [EIP-7954]: Increase Maximum Contract Size.
//!
//! From Amsterdam onward, deployed code may be up to 64 KiB ([EIP-170] allowed
//! 24 KiB) and init code up to 128 KiB ([EIP-3860] allowed 48 KiB). The init
//! code limit is enforced by the provider before the transaction reaches the
//! EVM, so the test covers both the provider's check and the EVM's.
//!
//! [EIP-7954]: https://eips.ethereum.org/EIPS/eip-7954
//! [EIP-170]: https://eips.ethereum.org/EIPS/eip-170
//! [EIP-3860]: https://eips.ethereum.org/EIPS/eip-3860

use std::num::NonZeroU64;

use edr_chain_l1::{rpc::TransactionRequest, L1ChainSpec};
use edr_defaults::SECRET_KEYS;
use edr_primitives::{eip3860, eip7954, Bytes, B256};
use edr_provider::{
    MethodInvocation, Provider, ProviderError, ProviderErrorForChainSpec, ProviderRequest,
    ResponseWithCallTraces, TransactionFailureReason,
};
use edr_test_utils::secret_key::secret_key_to_address;

use crate::common::{
    bytecode::opcode,
    provider::{code_at, new_provider_with_config, transaction_receipt},
};

/// EIP-8037 charges state gas per deployed byte from Amsterdam, so a maximum
/// size deployment needs far more than the 30M default block gas limit.
const AMSTERDAM_BLOCK_GAS_LIMIT: u64 = 200_000_000;
const AMSTERDAM_DEPLOYMENT_GAS: u64 = 150_000_000;

/// Init code returning `code_size` zero bytes of memory as the deployed code,
/// padded with unreachable trailing bytes up to `initcode_size`.
fn init_code(code_size: usize, initcode_size: usize) -> Bytes {
    // PUSH3 code_size; PUSH1 0; RETURN
    const CONSTRUCTOR_LEN: usize = 7;

    let [_, size0, size1, size2] = u32::try_from(code_size)
        .expect("code size fits a PUSH3")
        .to_be_bytes();
    let mut code = vec![
        opcode::PUSH3,
        size0,
        size1,
        size2,
        opcode::PUSH1,
        0x00,
        opcode::RETURN,
    ];
    assert_eq!(code.len(), CONSTRUCTOR_LEN);

    assert!(
        initcode_size >= CONSTRUCTOR_LEN,
        "init code must have room for the constructor"
    );
    code.resize(initcode_size, opcode::STOP);

    code.into()
}

fn new_provider(hardfork: edr_chain_l1::Hardfork) -> anyhow::Result<Provider<L1ChainSpec>> {
    new_provider_with_config(|config| {
        config.hardfork = hardfork;
        config.mining.block_gas_limit =
            Some(NonZeroU64::new(AMSTERDAM_BLOCK_GAS_LIMIT).expect("non-zero"));
        // Surface a halted deployment as an error instead of a failed receipt.
        config.bail_on_transaction_failure = true;
    })
}

/// Sends a deployment transaction with `init_code` via `eth_sendTransaction`.
fn send_deployment(
    provider: &Provider<L1ChainSpec>,
    init_code: Bytes,
) -> Result<ResponseWithCallTraces, ProviderErrorForChainSpec<L1ChainSpec>> {
    let request = TransactionRequest {
        from: secret_key_to_address(SECRET_KEYS[0]).expect("valid secret key"),
        data: Some(init_code),
        gas: Some(AMSTERDAM_DEPLOYMENT_GAS),
        ..TransactionRequest::default()
    };

    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(request),
    ))
}

fn assert_initcode_rejected(
    result: &Result<ResponseWithCallTraces, ProviderErrorForChainSpec<L1ChainSpec>>,
    max_initcode_size: usize,
) {
    let expected =
        format!("The max length allowed by the current hardfork is {max_initcode_size}.");
    assert!(
        matches!(
            result,
            Err(ProviderError::InvalidArgument(message)) if message.contains(&expected)
        ),
        "the provider should reject the init code, got {result:?}"
    );
}

fn assert_code_size_limit_hit(
    result: &Result<ResponseWithCallTraces, ProviderErrorForChainSpec<L1ChainSpec>>,
) {
    assert!(
        matches!(
            result,
            Err(ProviderError::TransactionFailed(failure))
                if matches!(
                    failure.failure.reason,
                    TransactionFailureReason::CreateContractSizeLimit
                )
        ),
        "the deployment should halt on the code size limit, got {result:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn amsterdam_deploys_max_code_from_max_initcode() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;

    let transaction_hash: B256 = send_deployment(
        &provider,
        init_code(eip7954::MAX_CODE_SIZE, eip7954::MAX_INITCODE_SIZE),
    )?
    .deserialize_result()?;

    let receipt = transaction_receipt(&provider, transaction_hash)?;
    assert_eq!(receipt.status, Some(true));

    let contract_address = receipt
        .contract_address
        .expect("deployment should create a contract");
    assert_eq!(
        code_at(&provider, contract_address)?.len(),
        eip7954::MAX_CODE_SIZE
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn amsterdam_rejects_initcode_above_limit() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;

    let result = send_deployment(
        &provider,
        init_code(eip7954::MAX_CODE_SIZE, eip7954::MAX_INITCODE_SIZE + 1),
    );

    assert_initcode_rejected(&result, eip7954::MAX_INITCODE_SIZE);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn amsterdam_rejects_code_above_limit() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::Amsterdam)?;

    let result = send_deployment(
        &provider,
        init_code(eip7954::MAX_CODE_SIZE + 1, eip7954::MAX_INITCODE_SIZE),
    );

    assert_code_size_limit_hit(&result);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn osaka_rejects_amsterdam_initcode_size() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::Osaka)?;

    let result = send_deployment(
        &provider,
        init_code(eip7954::MAX_CODE_SIZE, eip7954::MAX_INITCODE_SIZE),
    );

    assert_initcode_rejected(&result, eip3860::MAX_INITCODE_SIZE);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn osaka_rejects_amsterdam_code_size() -> anyhow::Result<()> {
    let provider = new_provider(edr_chain_l1::Hardfork::Osaka)?;

    let result = send_deployment(
        &provider,
        init_code(eip7954::MAX_CODE_SIZE, eip3860::MAX_INITCODE_SIZE),
    );

    assert_code_size_limit_hit(&result);

    Ok(())
}
