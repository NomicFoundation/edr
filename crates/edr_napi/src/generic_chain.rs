use std::sync::Arc;

use edr_eth::specification;
use edr_generic::GenericChainSpec;
use napi_derive::napi;

use crate::{
    logger::{Logger, LoggerConfig},
    provider::{self, factory::SyncProviderFactory, ProviderBuilder, ProviderFactory},
    spec::SyncNapiSpec,
    subscription::{SubscriptionCallback, SubscriptionConfig},
};

pub struct GenericChainProviderFactory;

impl SyncProviderFactory for GenericChainProviderFactory {
    fn create_provider_builder(
        &self,
        env: &napi::Env,
        provider_config: edr_napi_core::provider::Config,
        logger_config: LoggerConfig,
        subscription_config: SubscriptionConfig,
    ) -> napi::Result<Box<dyn provider::Builder>> {
        let logger = Logger::<GenericChainSpec>::new(env, logger_config)?;

        let provider_config =
            edr_provider::ProviderConfig::<GenericChainSpec>::from(provider_config);

        let subscription_callback =
            SubscriptionCallback::new(env, subscription_config.subscription_callback)?;

        Ok(Box::new(ProviderBuilder::new(
            logger,
            provider_config,
            subscription_callback,
        )))
    }
}

#[napi]
pub const GENERIC_CHAIN_TYPE: &str = GenericChainSpec::CHAIN_TYPE;

#[napi]
pub fn generic_chain_provider_factory() -> ProviderFactory {
    let factory: Arc<dyn SyncProviderFactory> = Arc::new(GenericChainProviderFactory);
    factory.into()
}

/// Identifier for the Ethereum spec.
#[napi]
pub enum SpecId {
    /// Frontier
    Frontier = 0,
    /// Frontier Thawing
    FrontierThawing = 1,
    /// Homestead
    Homestead = 2,
    /// DAO Fork
    DaoFork = 3,
    /// Tangerine
    Tangerine = 4,
    /// Spurious Dragon
    SpuriousDragon = 5,
    /// Byzantium
    Byzantium = 6,
    /// Constantinople
    Constantinople = 7,
    /// Petersburg
    Petersburg = 8,
    /// Istanbul
    Istanbul = 9,
    /// Muir Glacier
    MuirGlacier = 10,
    /// Berlin
    Berlin = 11,
    /// London
    London = 12,
    /// Arrow Glacier
    ArrowGlacier = 13,
    /// Gray Glacier
    GrayGlacier = 14,
    /// Merge
    Merge = 15,
    /// Shanghai
    Shanghai = 16,
    /// Cancun
    Cancun = 17,
    /// Latest
    Latest = 18,
}

impl From<SpecId> for edr_eth::SpecId {
    fn from(value: SpecId) -> Self {
        match value {
            SpecId::Frontier => edr_eth::SpecId::FRONTIER,
            SpecId::FrontierThawing => edr_eth::SpecId::FRONTIER_THAWING,
            SpecId::Homestead => edr_eth::SpecId::HOMESTEAD,
            SpecId::DaoFork => edr_eth::SpecId::DAO_FORK,
            SpecId::Tangerine => edr_eth::SpecId::TANGERINE,
            SpecId::SpuriousDragon => edr_eth::SpecId::SPURIOUS_DRAGON,
            SpecId::Byzantium => edr_eth::SpecId::BYZANTIUM,
            SpecId::Constantinople => edr_eth::SpecId::CONSTANTINOPLE,
            SpecId::Petersburg => edr_eth::SpecId::PETERSBURG,
            SpecId::Istanbul => edr_eth::SpecId::ISTANBUL,
            SpecId::MuirGlacier => edr_eth::SpecId::MUIR_GLACIER,
            SpecId::Berlin => edr_eth::SpecId::BERLIN,
            SpecId::London => edr_eth::SpecId::LONDON,
            SpecId::ArrowGlacier => edr_eth::SpecId::ARROW_GLACIER,
            SpecId::GrayGlacier => edr_eth::SpecId::GRAY_GLACIER,
            SpecId::Merge => edr_eth::SpecId::MERGE,
            SpecId::Shanghai => edr_eth::SpecId::SHANGHAI,
            SpecId::Cancun => edr_eth::SpecId::CANCUN,
            SpecId::Latest => edr_eth::SpecId::LATEST,
        }
    }
}

macro_rules! export_spec_id {
    ($($variant:ident),*) => {
        $(
            #[napi]
            pub const $variant: &str = specification::id::$variant;
        )*
    };
}

export_spec_id!(
    FRONTIER,
    FRONTIER_THAWING,
    HOMESTEAD,
    DAO_FORK,
    TANGERINE,
    SPURIOUS_DRAGON,
    BYZANTIUM,
    CONSTANTINOPLE,
    PETERSBURG,
    ISTANBUL,
    MUIR_GLACIER,
    BERLIN,
    LONDON,
    ARROW_GLACIER,
    GRAY_GLACIER,
    MERGE,
    SHANGHAI,
    CANCUN,
    PRAGUE,
    PRAGUE_EOF,
    LATEST
);
