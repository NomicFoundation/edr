#![cfg(feature = "test-utils")]

use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

use edr_chain_l1::L1ChainSpec;
use edr_provider::{
    test_utils::create_test_config, time::CurrentTime, Logger, MethodInvocation, NoopLogger,
    Provider, ProviderError, ProviderErrorForChainSpec, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use parking_lot::RwLock;
use tokio::runtime;

/// Generous: a correct implementation settles the callback immediately.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

/// A logger that panics once its method logs are printed, killing the event
/// loop's thread while a request is in flight.
#[derive(Clone)]
struct PanickingLogger;

impl Logger<L1ChainSpec, CurrentTime> for PanickingLogger {
    fn is_enabled(&self) -> bool {
        true
    }

    fn set_is_enabled(&mut self, _is_enabled: bool) {}

    fn print_method_logs(
        &mut self,
        _method: &str,
        _error: Option<&ProviderErrorForChainSpec<L1ChainSpec>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        panic!("logger panic");
    }
}

fn provider_with_logger(
    logger: Box<dyn edr_provider::SyncLogger<L1ChainSpec, CurrentTime>>,
) -> anyhow::Result<Provider<L1ChainSpec>> {
    Ok(Provider::new(
        runtime::Handle::current(),
        logger,
        Box::new(|_| ()),
        create_test_config(),
        Arc::new(RwLock::<ContractDecoder>::default()),
        CurrentTime,
    )?)
}

fn block_number_request() -> ProviderRequest<L1ChainSpec> {
    ProviderRequest::with_single(MethodInvocation::BlockNumber(()))
}

/// Settles a request through [`Provider::enqueue_request`], failing if the
/// callback is dropped instead of invoked.
///
/// The channel is kept connected by a second sender, so a dropped callback
/// blocks rather than reporting a disconnect. That distinguishes an invoked
/// callback from a dropped one, which is the distinction that matters: an
/// N-API `JsDeferred` does not settle its promise when dropped.
fn enqueue_and_await_response(
    provider: &Provider<L1ChainSpec>,
) -> anyhow::Result<Option<ProviderErrorForChainSpec<L1ChainSpec>>> {
    let (sender, receiver) = mpsc::channel();
    let _keep_connected = sender.clone();

    provider.enqueue_request(
        block_number_request(),
        Box::new(move |response| {
            let _ = sender.send(response.err());
        }),
    );

    Ok(receiver.recv_timeout(RESPONSE_TIMEOUT)?)
}

/// A panic while a request is in flight must settle that request. Its callback
/// is moved into the event loop's frame, so an unwind would otherwise drop it
/// without invoking it.
#[tokio::test(flavor = "multi_thread")]
async fn request_in_flight_is_settled_when_the_event_loop_panics() -> anyhow::Result<()> {
    let provider = provider_with_logger(Box::new(PanickingLogger))?;

    let error = enqueue_and_await_response(&provider)?
        .expect("the logger panics while the request is in flight");

    assert!(
        matches!(error, ProviderError::UnexpectedTermination),
        "expected UnexpectedTermination, got {error:?}"
    );

    Ok(())
}

/// Once the event loop has terminated, an enqueued request is settled rather
/// than discarded with its callback.
#[tokio::test(flavor = "multi_thread")]
async fn enqueued_request_is_settled_after_termination() -> anyhow::Result<()> {
    let provider = provider_with_logger(Box::new(PanickingLogger))?;

    // Kills the event loop.
    let _ = provider.handle_request(block_number_request());

    let error =
        enqueue_and_await_response(&provider)?.expect("the enqueued request cannot be handled");

    assert!(
        matches!(error, ProviderError::UnexpectedTermination),
        "expected UnexpectedTermination, got {error:?}"
    );

    Ok(())
}

/// The blocking API reports a terminated event loop instead of panicking on the
/// caller's thread.
#[tokio::test(flavor = "multi_thread")]
async fn blocking_request_reports_termination() -> anyhow::Result<()> {
    let provider = provider_with_logger(Box::new(PanickingLogger))?;

    let error = provider
        .handle_request(block_number_request())
        .expect_err("the logger panics while the request is in flight");

    assert!(
        matches!(error, ProviderError::UnexpectedTermination),
        "expected UnexpectedTermination, got {error:?}"
    );

    Ok(())
}

/// A healthy event loop invokes the callback with the response.
#[tokio::test(flavor = "multi_thread")]
async fn healthy_provider_settles_requests() -> anyhow::Result<()> {
    let provider = provider_with_logger(Box::<NoopLogger<L1ChainSpec>>::default())?;

    assert!(
        enqueue_and_await_response(&provider)?.is_none(),
        "the request should succeed"
    );
    provider.handle_request(block_number_request())?;

    Ok(())
}
