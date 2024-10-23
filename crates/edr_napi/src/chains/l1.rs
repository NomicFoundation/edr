use std::sync::Arc;

use edr_eth::l1::L1ChainSpec;
use edr_napi_core::{
    logger::Logger,
    provider::{self, ProviderBuilder, SyncProviderFactory},
    spec::SyncNapiSpec as _,
    subscription,
};
use napi_derive::napi;

use crate::provider::ProviderFactory;

pub struct L1ProviderFactory;

impl SyncProviderFactory for L1ProviderFactory {
    fn create_provider_builder(
        &self,
        env: &napi::Env,
        provider_config: edr_napi_core::provider::Config,
        logger_config: edr_napi_core::logger::Config,
        subscription_config: edr_napi_core::subscription::Config,
    ) -> napi::Result<Box<dyn provider::Builder>> {
        let logger = Logger::<L1ChainSpec>::new(logger_config)?;

        let provider_config = edr_provider::ProviderConfig::<L1ChainSpec>::from(provider_config);

        let subscription_callback =
            subscription::Callback::new(env, subscription_config.subscription_callback)?;

        Ok(Box::new(ProviderBuilder::new(
            Box::new(logger),
            provider_config,
            subscription_callback,
        )))
    }
}

#[napi]
pub const L1_CHAIN_TYPE: &str = L1ChainSpec::CHAIN_TYPE;

#[napi]
pub fn l1_provider_factory() -> ProviderFactory {
    let factory: Arc<dyn SyncProviderFactory> = Arc::new(L1ProviderFactory);
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

impl From<SpecId> for edr_eth::l1::SpecId {
    fn from(value: SpecId) -> Self {
        match value {
            SpecId::Frontier => edr_eth::l1::SpecId::FRONTIER,
            SpecId::FrontierThawing => edr_eth::l1::SpecId::FRONTIER_THAWING,
            SpecId::Homestead => edr_eth::l1::SpecId::HOMESTEAD,
            SpecId::DaoFork => edr_eth::l1::SpecId::DAO_FORK,
            SpecId::Tangerine => edr_eth::l1::SpecId::TANGERINE,
            SpecId::SpuriousDragon => edr_eth::l1::SpecId::SPURIOUS_DRAGON,
            SpecId::Byzantium => edr_eth::l1::SpecId::BYZANTIUM,
            SpecId::Constantinople => edr_eth::l1::SpecId::CONSTANTINOPLE,
            SpecId::Petersburg => edr_eth::l1::SpecId::PETERSBURG,
            SpecId::Istanbul => edr_eth::l1::SpecId::ISTANBUL,
            SpecId::MuirGlacier => edr_eth::l1::SpecId::MUIR_GLACIER,
            SpecId::Berlin => edr_eth::l1::SpecId::BERLIN,
            SpecId::London => edr_eth::l1::SpecId::LONDON,
            SpecId::ArrowGlacier => edr_eth::l1::SpecId::ARROW_GLACIER,
            SpecId::GrayGlacier => edr_eth::l1::SpecId::GRAY_GLACIER,
            SpecId::Merge => edr_eth::l1::SpecId::MERGE,
            SpecId::Shanghai => edr_eth::l1::SpecId::SHANGHAI,
            SpecId::Cancun => edr_eth::l1::SpecId::CANCUN,
            SpecId::Latest => edr_eth::l1::SpecId::LATEST,
        }
    }
}

macro_rules! export_spec_id {
    ($($variant:ident),*) => {
        $(
            #[napi]
            pub const $variant: &str = edr_eth::l1::hardfork::id::$variant;
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
