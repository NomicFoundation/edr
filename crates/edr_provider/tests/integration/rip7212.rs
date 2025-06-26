#![cfg(feature = "test-utils")]

use std::sync::Arc;

use edr_eth::{bytes, l1::L1ChainSpec, signature::public_key_to_address, Bytes, HashMap};
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, MethodInvocation, NoopLogger, Provider,
    ProviderRequest,
};
use edr_rpc_eth::{CallRequest, TransactionRequest};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_test_utils::secret_key::secret_key_from_str;
use revm_precompile::secp256r1;
use tokio::runtime;

// Example adapted from
// <https://github.com/maticnetwork/bor/blob/bade7f57df5c09ae060c15fc66aed488c526149e/core/vm/testdata/precompiles/p256Verify.json>
static CALLDATA: Bytes = bytes!(
    "4cee90eb86eaa050036147a12d49004b6b9c72bd725d39d4785011fe190f0b4da73bd4903f0ce3b639bbbf6e8e80d16931ff4bcf5993d58468e8fb19086e8cac36dbcd03009df8c59286b162af3bd7fcc0450c9aa81be5d10d312af6c66b1d604aebd3099c618202fcfe16ae7770b0c49ab5eadf74b754204a3bb6060e44eff37618b065f9832de4ca6ca971a7a1adc826d0f7c00181a5fb2ddf79ae00b4e10e"
);

#[tokio::test(flavor = "multi_thread")]
async fn rip7212_disabled() -> anyhow::Result<()> {
    let config = create_test_config(); // default config, no custom precompiles

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let response =
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            CallRequest {
                to: Some(*secp256r1::P256VERIFY.address()),
                data: Some(CALLDATA.clone()),
                ..CallRequest::default()
            },
            None,
            None,
        )))?;

    assert_eq!(response.result, "0x");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn rip7212_enabled() -> anyhow::Result<()> {
    let mut config = create_test_config();
    config.precompile_overrides = HashMap::from([(
        *secp256r1::P256VERIFY.address(),
        *secp256r1::P256VERIFY.precompile(),
    )]);

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let response =
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::Call(
            CallRequest {
                to: Some(*secp256r1::P256VERIFY.address()),
                data: Some(CALLDATA.clone()),
                ..CallRequest::default()
            },
            None,
            None,
        )))?;

    // 1 gwei in hex
    assert_eq!(
        response.result,
        "0x0000000000000000000000000000000000000000000000000000000000000001"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn rip7212_enabled_send_transaction() -> anyhow::Result<()> {
    let mut config = create_test_config();
    config.precompile_overrides = HashMap::from([(
        *secp256r1::P256VERIFY.address(),
        *secp256r1::P256VERIFY.precompile(),
    )]);

    let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
    let sender = public_key_to_address(secret_key.public_key());

    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    let provider = Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )?;

    let response = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(TransactionRequest {
            from: sender,
            to: Some(*secp256r1::P256VERIFY.address()),
            data: Some(CALLDATA.clone()),
            ..TransactionRequest::default()
        }),
    ))?;

    assert_eq!(response.result, "0x");

    Ok(())
}
