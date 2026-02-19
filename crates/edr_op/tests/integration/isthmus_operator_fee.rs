use std::sync::Arc;

use edr_chain_l1::rpc::TransactionRequest;
use edr_defaults::SECRET_KEYS;
use edr_op::{predeploys::L1_BLOCK_PREDEPLOY_ADDRESS, OpChainSpec};
use edr_primitives::{address, B256, U256};
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, MethodInvocation, NoopLogger, Provider,
    ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::secret_key::secret_key_to_address;
use parking_lot::RwLock;
use tokio::runtime;

const OPERATOR_FEE_STORAGE_INDEX: u64 = 8;

fn create_isthmus_provider() -> anyhow::Result<Provider<OpChainSpec>> {
    let config = {
        let mut config = create_test_config::<edr_op::Hardfork>();
        config.hardfork = edr_op::Hardfork::ISTHMUS;
        config
    };
    let logger = Box::new(NoopLogger::<OpChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?;
    Ok(provider)
}

#[tokio::test(flavor = "multi_thread")]
async fn operator_fee_parameters_storage_defaults_to_0() -> anyhow::Result<()> {
    let provider = create_isthmus_provider()?;

    let operator_fee = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetStorageAt(
            L1_BLOCK_PREDEPLOY_ADDRESS,
            U256::from(OPERATOR_FEE_STORAGE_INDEX),
            None,
        ),
    ))?;

    assert_eq!(operator_fee.result, format!("{:#066x}", U256::ZERO),);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn receipts_includes_operator_fee_constant_if_present() -> anyhow::Result<()> {
    let provider = create_isthmus_provider()?;

    let operator_fee_constant = 250;
    set_operator_fee_params_in_storage(&provider, 0, operator_fee_constant)?;

    let transaction_hash = send_transaction(&provider)?;
    let receipt = get_transaction_receipt(&provider, transaction_hash)?;

    assert_eq!(
        receipt.l1_block_info.operator_fee_constant,
        Some(u128::from(operator_fee_constant))
    );
    assert_eq!(
        receipt.l1_block_info.operator_fee_scalar,
        Some(u128::from(0u32))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn receipts_includes_operator_fee_scalar_if_present() -> anyhow::Result<()> {
    let provider = create_isthmus_provider()?;

    let operator_fee_scalar = 27;
    set_operator_fee_params_in_storage(&provider, operator_fee_scalar, 0)?;

    let transaction_hash = send_transaction(&provider)?;
    let receipt = get_transaction_receipt(&provider, transaction_hash)?;

    assert_eq!(
        receipt.l1_block_info.operator_fee_constant,
        Some(u128::from(0u32))
    );
    assert_eq!(
        receipt.l1_block_info.operator_fee_scalar,
        Some(u128::from(operator_fee_scalar))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn receipts_includes_operator_fee_scalar_and_constant_if_both_present() -> anyhow::Result<()>
{
    let provider = create_isthmus_provider()?;

    let operator_fee_constant = 316;
    let operator_fee_scalar = 15;
    set_operator_fee_params_in_storage(&provider, operator_fee_scalar, operator_fee_constant)?;

    let transaction_hash = send_transaction(&provider)?;
    let receipt = get_transaction_receipt(&provider, transaction_hash)?;

    assert_eq!(
        receipt.l1_block_info.operator_fee_constant,
        Some(u128::from(operator_fee_constant))
    );
    assert_eq!(
        receipt.l1_block_info.operator_fee_scalar,
        Some(u128::from(operator_fee_scalar))
    );

    Ok(())
}
#[tokio::test(flavor = "multi_thread")]
async fn receipts_does_not_include_operator_fee_params_if_absent() -> anyhow::Result<()> {
    let provider = create_isthmus_provider()?;
    let transaction_hash = send_transaction(&provider)?;
    let receipt = get_transaction_receipt(&provider, transaction_hash)?;

    // Isthmus specification says
    // > After Isthmus activation, 2 new fields operatorFeeScalar and
    // > operatorFeeConstant are added to transaction receipts if and only if at
    // > least one of them is non zero.
    // However revm is including them even when both are 0
    assert_eq!(receipt.l1_block_info.operator_fee_constant, Some(0));
    assert_eq!(receipt.l1_block_info.operator_fee_scalar, Some(0));

    Ok(())
}

fn get_transaction_receipt(
    provider: &Provider<OpChainSpec>,
    transaction_hash: B256,
) -> anyhow::Result<edr_op::rpc::OpRpcBlockReceipt> {
    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetTransactionReceipt(transaction_hash),
    ))?;

    let receipt: edr_op::rpc::OpRpcBlockReceipt = serde_json::from_value(result.result)?;
    Ok(receipt)
}

fn send_transaction(provider: &Provider<OpChainSpec>) -> anyhow::Result<B256> {
    let caller = secret_key_to_address(SECRET_KEYS[0])?;
    let transaction = TransactionRequest {
        from: caller,
        to: Some(address!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
        ..TransactionRequest::default()
    };

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(transaction),
    ))?;

    let transaction_hash: B256 = serde_json::from_value(result.result)?;
    Ok(transaction_hash)
}
fn set_operator_fee_params_in_storage(
    provider: &Provider<OpChainSpec>,
    operator_fee_scalar: u32,
    operator_fee_constant: u64,
) -> anyhow::Result<()> {
    provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SetStorageAt(
            L1_BLOCK_PREDEPLOY_ADDRESS,
            U256::from(OPERATOR_FEE_STORAGE_INDEX),
            encode_operator_fee_params(operator_fee_scalar, operator_fee_constant),
        ),
    ))?;
    Ok(())
}
fn encode_operator_fee_params(operator_fee_scalar: u32, operator_fee_constant: u64) -> U256 {
    let scalar: [u8; 4] = operator_fee_scalar.to_be_bytes();
    let constant: [u8; 8] = operator_fee_constant.to_be_bytes();

    let mut operator_fee = [0u8; 32];
    operator_fee[20..=23].copy_from_slice(&scalar);
    operator_fee[24..=31].copy_from_slice(&constant);

    U256::from_be_bytes(operator_fee)
}
