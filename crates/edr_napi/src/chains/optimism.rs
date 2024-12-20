use std::sync::Arc;

use edr_napi_core::{
    logger::{self, Logger},
    provider::{self, ProviderBuilder, SyncProviderFactory},
    spec::SyncNapiSpec as _,
    subscription,
};
use edr_optimism::{OptimismChainSpec, OptimismSpecId};
use napi_derive::napi;

use crate::{account::Account, provider::ProviderFactory};

pub struct OptimismProviderFactory;

impl SyncProviderFactory for OptimismProviderFactory {
    fn create_provider_builder(
        &self,
        env: &napi::Env,
        provider_config: provider::Config,
        logger_config: logger::Config,
        subscription_config: subscription::Config,
    ) -> napi::Result<Box<dyn provider::Builder>> {
        let logger = Logger::<OptimismChainSpec>::new(logger_config)?;

        let provider_config = edr_provider::ProviderConfig::<OptimismSpecId>::from(provider_config);

        let subscription_callback =
            subscription::Callback::new(env, subscription_config.subscription_callback)?;

        Ok(Box::new(ProviderBuilder::new(
            Box::new(logger),
            provider_config,
            subscription_callback,
        )))
    }
}

/// Enumeration of supported Optimism hardforks.
#[napi]
pub enum OptimismHardfork {
    Bedrock = 16,
    Regolith = 17,
    Shanghai = 18,
    Canyon = 19,
    Cancun = 20,
    Ecotone = 21,
    Fjord = 22,
    Granite = 23,
    Latest = 2_147_483_647, // Maximum value of i32
}

impl From<OptimismHardfork> for OptimismSpecId {
    fn from(hardfork: OptimismHardfork) -> Self {
        match hardfork {
            OptimismHardfork::Bedrock => OptimismSpecId::BEDROCK,
            OptimismHardfork::Regolith => OptimismSpecId::REGOLITH,
            OptimismHardfork::Shanghai => OptimismSpecId::SHANGHAI,
            OptimismHardfork::Canyon => OptimismSpecId::CANYON,
            OptimismHardfork::Cancun => OptimismSpecId::CANCUN,
            OptimismHardfork::Ecotone => OptimismSpecId::ECOTONE,
            OptimismHardfork::Fjord => OptimismSpecId::FJORD,
            OptimismHardfork::Granite => OptimismSpecId::GRANITE,
            OptimismHardfork::Latest => OptimismSpecId::LATEST,
        }
    }
}

impl TryFrom<&str> for OptimismHardfork {
    type Error = napi::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            edr_optimism::hardfork::id::BEDROCK => Ok(OptimismHardfork::Bedrock),
            edr_optimism::hardfork::id::REGOLITH => Ok(OptimismHardfork::Regolith),
            edr_optimism::hardfork::id::SHANGHAI => Ok(OptimismHardfork::Shanghai),
            edr_optimism::hardfork::id::CANYON => Ok(OptimismHardfork::Canyon),
            edr_optimism::hardfork::id::CANCUN => Ok(OptimismHardfork::Cancun),
            edr_optimism::hardfork::id::ECOTONE => Ok(OptimismHardfork::Ecotone),
            edr_optimism::hardfork::id::FJORD => Ok(OptimismHardfork::Fjord),
            edr_optimism::hardfork::id::GRANITE => Ok(OptimismHardfork::Granite),
            edr_optimism::hardfork::id::LATEST => Ok(OptimismHardfork::Latest),
            _ => Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("The provided Optimism hardfork `{value}` is not supported."),
            )),
        }
    }
}

#[napi]
pub const OPTIMISM_CHAIN_TYPE: &str = OptimismChainSpec::CHAIN_TYPE;

#[napi]
pub fn optimism_genesis_state(_hardfork: OptimismHardfork) -> Vec<Account> {
    Vec::new()
}

/// Tries to parse the provided string to create an instance of
/// [`OptimismHardfork`].
#[napi]
pub fn optimism_hardfork_from_string(hardfork: String) -> napi::Result<OptimismHardfork> {
    OptimismHardfork::try_from(hardfork.as_str())
}

#[napi]
pub fn optimism_provider_factory() -> ProviderFactory {
    let factory: Arc<dyn SyncProviderFactory> = Arc::new(OptimismProviderFactory);
    factory.into()
}

macro_rules! export_spec_id {
    ($($variant:ident),*) => {
        $(
            #[napi]
            pub const $variant: &str = edr_optimism::hardfork::id::$variant;
        )*
    };
}

// LATEST is not included because it is already included in l1.rs
export_spec_id! {
    BEDROCK,
    REGOLITH,
    SHANGHAI,
    CANYON,
    CANCUN,
    ECOTONE,
    FJORD,
    GRANITE
}
