use std::sync::Arc;

use edr_eth::{
    chain_spec::L1ChainSpec, result::InvalidTransaction, transaction::TransactionValidation,
};
use edr_napi_core::{
    logger::Logger, provider::SyncProviderFactory, subscription::SubscriptionCallback,
};
use edr_provider::{time::CurrentTime, SyncProviderSpec};
use napi::tokio::runtime;
use napi_derive::napi;

use super::{factory::ProviderFactory, SyncProvider};

/// Indicates that the EVM has experienced an exceptional halt. This causes
/// execution to immediately end with all gas being consumed.
#[napi]
pub enum ExceptionalHalt {
    OutOfGas,
    OpcodeNotFound,
    InvalidEFOpcode,
    InvalidJump,
    NotActivated,
    StackUnderflow,
    StackOverflow,
    OutOfOffset,
    CreateCollision,
    PrecompileError,
    NonceOverflow,
    /// Create init code size exceeds limit (runtime).
    CreateContractSizeLimit,
    /// Error on created contract that begins with EF
    CreateContractStartingWithEF,
    /// EIP-3860: Limit and meter initcode. Initcode size limit exceeded.
    CreateInitCodeSizeLimit,
    /// Aux data overflow, new aux data is larger tha u16 max size.
    EofAuxDataOverflow,
    /// Aud data is smaller then already present data size.
    EofAuxDataTooSmall,
    /// EOF Subroutine stack overflow
    EOFFunctionStackOverflow,
}

pub struct ProviderBuilder<ChainSpecT: SyncProviderSpec<CurrentTime>> {
    logger: Logger<ChainSpecT>,
    provider_config: edr_provider::ProviderConfig<ChainSpecT>,
    subscription_callback: SubscriptionCallback<ChainSpecT>,
}

impl<
        ChainSpecT: SyncProviderSpec<
            CurrentTime,
            Block: Default,
            Transaction: Default
                             + TransactionValidation<
                ValidationError: From<InvalidTransaction> + PartialEq,
            >,
        >,
    > edr_napi_core::provider::Builder for ProviderBuilder<ChainSpecT>
{
    fn build(self: Box<Self>, runtime: runtime::Handle) -> napi::Result<Arc<dyn SyncProvider>> {
        let provider = edr_provider::Sequential::<ChainSpecT>::new(
            runtime.clone(),
            Box::new(self.logger),
            Box::new(move |event| self.subscription_callback.call(event)),
            self.provider_config,
            CurrentTime,
        )
        .map_err(|error| napi::Error::new(napi::Status::GenericFailure, error.to_string()))?;

        Ok(Arc::new(provider))
    }
}

pub struct L1ProviderFactory;

impl SyncProviderFactory for L1ProviderFactory {
    fn create_provider_builder(
        &self,
        env: &napi::Env,
        provider_config: edr_napi_core::provider::Config,
        logger_config: edr_napi_core::logger::LoggerConfig,
        subscription_config: edr_napi_core::subscription::SubscriptionConfig,
    ) -> napi::Result<Box<dyn edr_napi_core::provider::Builder>> {
        let logger = Logger::<L1ChainSpec>::new(env, logger_config)?;

        let provider_config =
            edr_provider::ProviderConfig::<L1ChainSpec>::try_from(provider_config)?;

        let subscription_callback =
            SubscriptionCallback::new(env, subscription_config.subscription_callback)?;

        Ok(Box::new(ProviderBuilder {
            logger,
            provider_config,
            subscription_callback,
        }))
    }
}

#[napi]
pub fn l1_provider_factory() -> ProviderFactory {
    let factory: Arc<dyn SyncProviderFactory> = Arc::new(L1ProviderFactory);
    factory.into()
}
