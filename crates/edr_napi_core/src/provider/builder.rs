use std::sync::Arc;

use edr_eth::B256;
use edr_evm::blockchain::BlockchainErrorForChainSpec;
use edr_provider::{time::CurrentTime, SyncLogger};
use edr_solidity::contract_decoder::ContractDecoder;
use napi::tokio::runtime;

use crate::{
    provider::SyncProvider,
    spec::SyncNapiSpec,
    subscription::{self, SubscriptionEvent},
};

/// A builder for creating a new provider.
pub trait Builder: Send {
    /// Consumes the builder and returns a new provider.
    fn build(self: Box<Self>, runtime: runtime::Handle) -> napi::Result<Arc<dyn SyncProvider>>;
}

pub struct ProviderBuilder<ChainSpecT: SyncNapiSpec<CurrentTime>> {
    contract_decoder: Arc<ContractDecoder>,
    logger: Box<
        dyn SyncLogger<
            ChainSpecT,
            CurrentTime,
            BlockchainError = BlockchainErrorForChainSpec<ChainSpecT>,
        >,
    >,
    provider_config: edr_provider::ProviderConfig<ChainSpecT::Hardfork>,
    subscription_callback: subscription::Callback,
}

impl<ChainSpecT: SyncNapiSpec<CurrentTime>> ProviderBuilder<ChainSpecT> {
    /// Constructs a new instance.
    pub fn new(
        contract_decoder: Arc<ContractDecoder>,
        logger: Box<
            dyn SyncLogger<
                ChainSpecT,
                CurrentTime,
                BlockchainError = BlockchainErrorForChainSpec<ChainSpecT>,
            >,
        >,
        provider_config: edr_provider::ProviderConfig<ChainSpecT::Hardfork>,
        subscription_callback: subscription::Callback,
    ) -> Self {
        Self {
            contract_decoder,
            logger,
            provider_config,
            subscription_callback,
        }
    }
}

impl<ChainSpecT: SyncNapiSpec<CurrentTime>> Builder for ProviderBuilder<ChainSpecT> {
    fn build(self: Box<Self>, runtime: runtime::Handle) -> napi::Result<Arc<dyn SyncProvider>> {
        let builder = *self;

        let provider = edr_provider::Provider::<ChainSpecT>::new(
            runtime.clone(),
            builder.logger,
            Box::new(move |event| {
                let event = SubscriptionEvent::new::<
                    ChainSpecT::Block,
                    ChainSpecT::RpcBlock<B256>,
                    ChainSpecT::SignedTransaction,
                >(event);

                builder.subscription_callback.call(event);
            }),
            builder.provider_config,
            builder.contract_decoder,
            CurrentTime,
        )
        .map_err(|error| napi::Error::new(napi::Status::GenericFailure, error.to_string()))?;

        Ok(Arc::new(provider))
    }
}
