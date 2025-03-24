#![cfg(feature = "test-utils")]

use std::sync::Arc;

use edr_eth::{address, bytes, l1::L1ChainSpec, signature::public_key_to_address, U256};
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, MethodInvocation, NoopLogger, Provider,
    ProviderRequest,
};
use edr_rpc_eth::CallRequest;
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

#[tokio::test(flavor = "multi_thread")]
async fn with_value() -> anyhow::Result<()> {
    let mut config = create_test_config();

    let from = {
        let account = config.accounts.first_mut().expect("should have an account");

        // Lower the balance to trigger an `OutOfFunds` error in REVM
        account.balance = U256::from(0xau64);

        public_key_to_address(account.secret_key.public_key())
    };

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

    // {"method":"eth_estimateGas","params":[{"gas":"0x186a0","gasPrice":null,"maxFeePerGas":null,"maxPriorityFeePerGas":null,"value":"0xa","data":"0x3e6fec0490f79bf6eb2c4f870365e785982e1f101e93b906","accessList":null,"type":null,"blobs":null,"blobHashes":null},"pending"]}

    let _response =
        provider.handle_request(ProviderRequest::Single(MethodInvocation::EstimateGas(
            CallRequest {
                from: Some(from),
                to: Some(address!("0xdf951d2061b12922bfbf22cb17b17f3b39183570")),
                value: Some(U256::from(0xau64)),
                data: Some(bytes!("0x3e6fec0490f79bf6eb2c4f870365e785982e1f101e93b906")),
                ..CallRequest::default()
            },
            None,
        )))?;

    Ok(())
}
