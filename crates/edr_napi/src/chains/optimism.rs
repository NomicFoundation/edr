use std::{str::FromStr, sync::Arc};

use edr_napi_core::{
    logger::{self, Logger},
    provider::{self, ProviderBuilder, SyncProviderFactory},
    spec::SyncNapiSpec as _,
    subscription,
};
use edr_optimism::{OpChainSpec, OpSpecId};
use edr_solidity::contract_decoder::ContractDecoder;
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
        contract_decoder: Arc<ContractDecoder>,
    ) -> napi::Result<Box<dyn provider::Builder>> {
        let logger = Logger::<OpChainSpec>::new(logger_config, Arc::clone(&contract_decoder))?;

        let provider_config = edr_provider::ProviderConfig::<OpSpecId>::try_from(provider_config)?;

        let subscription_callback =
            subscription::Callback::new(env, subscription_config.subscription_callback)?;

        Ok(Box::new(ProviderBuilder::new(
            contract_decoder,
            Box::new(logger),
            provider_config,
            subscription_callback,
        )))
    }
}

/// Enumeration of supported Optimism hardforks.
#[napi]
pub enum OptimismHardfork {
    Bedrock = 100,
    Regolith = 101,
    Canyon = 102,
    Ecotone = 103,
    Fjord = 104,
    Granite = 105,
}

impl From<OptimismHardfork> for OpSpecId {
    fn from(hardfork: OptimismHardfork) -> Self {
        match hardfork {
            OptimismHardfork::Bedrock => OpSpecId::BEDROCK,
            OptimismHardfork::Regolith => OpSpecId::REGOLITH,
            OptimismHardfork::Canyon => OpSpecId::CANYON,
            OptimismHardfork::Ecotone => OpSpecId::ECOTONE,
            OptimismHardfork::Fjord => OpSpecId::FJORD,
            OptimismHardfork::Granite => OpSpecId::GRANITE,
        }
    }
}

impl FromStr for OptimismHardfork {
    type Err = napi::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            edr_optimism::hardfork::name::BEDROCK => Ok(OptimismHardfork::Bedrock),
            edr_optimism::hardfork::name::REGOLITH => Ok(OptimismHardfork::Regolith),
            edr_optimism::hardfork::name::CANYON => Ok(OptimismHardfork::Canyon),
            edr_optimism::hardfork::name::ECOTONE => Ok(OptimismHardfork::Ecotone),
            edr_optimism::hardfork::name::FJORD => Ok(OptimismHardfork::Fjord),
            edr_optimism::hardfork::name::GRANITE => Ok(OptimismHardfork::Granite),
            _ => Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("The provided Optimism hardfork `{s}` is not supported."),
            )),
        }
    }
}

/// Tries to parse the provided string to create an [`OptimismHardfork`]
/// instance.
///
/// Returns an error if the string does not match any known hardfork.
#[napi]
pub fn optimism_hardfork_from_string(hardfork: String) -> napi::Result<OptimismHardfork> {
    hardfork.parse()
}

/// Returns the string representation of the provided Optimism hardfork.
#[napi]
pub fn optimism_hardfork_to_string(hardfork: OptimismHardfork) -> &'static str {
    match hardfork {
        OptimismHardfork::Bedrock => edr_optimism::hardfork::name::BEDROCK,
        OptimismHardfork::Regolith => edr_optimism::hardfork::name::REGOLITH,
        OptimismHardfork::Canyon => edr_optimism::hardfork::name::CANYON,
        OptimismHardfork::Ecotone => edr_optimism::hardfork::name::ECOTONE,
        OptimismHardfork::Fjord => edr_optimism::hardfork::name::FJORD,
        OptimismHardfork::Granite => edr_optimism::hardfork::name::GRANITE,
    }
}

/// Returns the latest supported OP hardfork.
///
/// The returned value will be updated after each network upgrade.
#[napi]
pub fn optimism_latest_hardfork() -> OptimismHardfork {
    OptimismHardfork::Granite
}

#[napi]
pub const OPTIMISM_CHAIN_TYPE: &str = OpChainSpec::CHAIN_TYPE;

#[napi]
pub fn optimism_genesis_state(_hardfork: OptimismHardfork) -> Vec<Account> {
    Vec::new()
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
            pub const $variant: &str = edr_optimism::hardfork::name::$variant;
        )*
    };
}

export_spec_id! {
    BEDROCK,
    REGOLITH,
    CANYON,
    ECOTONE,
    FJORD,
    GRANITE
}
