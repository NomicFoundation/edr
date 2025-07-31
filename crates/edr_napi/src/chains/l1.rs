use std::{str::FromStr, sync::Arc};

use edr_eth::l1::{self, L1ChainSpec};
use edr_evm::eips::{
    eip2935::{HISTORY_STORAGE_ADDRESS, HISTORY_STORAGE_UNSUPPORTED_BYTECODE},
    eip4788::{BEACON_ROOTS_ADDRESS, BEACON_ROOTS_BYTECODE},
};
use edr_napi_core::{
    logger::Logger,
    provider::{self, ProviderBuilder, SyncProviderFactory},
    subscription,
};
use edr_provider::time::CurrentTime;
use edr_solidity::contract_decoder::ContractDecoder;
use napi::bindgen_prelude::{BigInt, Uint8Array};
use napi_derive::napi;

use crate::{account::AccountOverride, provider::ProviderFactory};

pub struct L1ProviderFactory;

impl SyncProviderFactory for L1ProviderFactory {
    fn create_provider_builder(
        &self,
        env: &napi::Env,
        provider_config: edr_napi_core::provider::Config,
        logger_config: edr_napi_core::logger::Config,
        subscription_config: edr_napi_core::subscription::Config,
        contract_decoder: Arc<ContractDecoder>,
    ) -> napi::Result<Box<dyn provider::Builder>> {
        let logger =
            Logger::<L1ChainSpec, CurrentTime>::new(logger_config, Arc::clone(&contract_decoder))?;

        let provider_config =
            edr_provider::ProviderConfig::<l1::SpecId>::try_from(provider_config)?;

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

#[napi]
pub const L1_CHAIN_TYPE: &str = edr_eth::l1::CHAIN_TYPE;

#[napi(catch_unwind)]
pub fn l1_genesis_state(hardfork: SpecId) -> Vec<AccountOverride> {
    // Use closures for lazy execution
    let beacon_roots_account_constructor = || AccountOverride {
        address: Uint8Array::with_data_copied(BEACON_ROOTS_ADDRESS),
        balance: Some(BigInt::from(0u64)),
        nonce: Some(BigInt::from(0u64)),
        code: Some(Uint8Array::with_data_copied(&BEACON_ROOTS_BYTECODE)),
        storage: Some(Vec::new()),
    };

    let history_storage_account_constructor = || AccountOverride {
        address: Uint8Array::with_data_copied(HISTORY_STORAGE_ADDRESS),
        balance: Some(BigInt::from(0u64)),
        nonce: Some(BigInt::from(0u64)),
        code: Some(Uint8Array::with_data_copied(
            &HISTORY_STORAGE_UNSUPPORTED_BYTECODE,
        )),
        storage: Some(Vec::new()),
    };

    if hardfork < SpecId::Cancun {
        Vec::new()
    } else if hardfork < SpecId::Prague {
        vec![beacon_roots_account_constructor()]
    } else {
        vec![
            beacon_roots_account_constructor(),
            history_storage_account_constructor(),
        ]
    }
}

#[napi(catch_unwind)]
pub fn l1_provider_factory() -> ProviderFactory {
    let factory: Arc<dyn SyncProviderFactory> = Arc::new(L1ProviderFactory);
    factory.into()
}

/// Identifier for the Ethereum spec.
#[napi]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
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
    /// Prague
    Prague = 18,
}

impl FromStr for SpecId {
    type Err = napi::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            edr_eth::l1::hardfork::name::FRONTIER => Ok(SpecId::Frontier),
            edr_eth::l1::hardfork::name::FRONTIER_THAWING => Ok(SpecId::FrontierThawing),
            edr_eth::l1::hardfork::name::HOMESTEAD => Ok(SpecId::Homestead),
            edr_eth::l1::hardfork::name::DAO_FORK => Ok(SpecId::DaoFork),
            edr_eth::l1::hardfork::name::TANGERINE => Ok(SpecId::Tangerine),
            edr_eth::l1::hardfork::name::SPURIOUS_DRAGON => Ok(SpecId::SpuriousDragon),
            edr_eth::l1::hardfork::name::BYZANTIUM => Ok(SpecId::Byzantium),
            edr_eth::l1::hardfork::name::CONSTANTINOPLE => Ok(SpecId::Constantinople),
            edr_eth::l1::hardfork::name::PETERSBURG => Ok(SpecId::Petersburg),
            edr_eth::l1::hardfork::name::ISTANBUL => Ok(SpecId::Istanbul),
            edr_eth::l1::hardfork::name::MUIR_GLACIER => Ok(SpecId::MuirGlacier),
            edr_eth::l1::hardfork::name::BERLIN => Ok(SpecId::Berlin),
            edr_eth::l1::hardfork::name::LONDON => Ok(SpecId::London),
            edr_eth::l1::hardfork::name::ARROW_GLACIER => Ok(SpecId::ArrowGlacier),
            edr_eth::l1::hardfork::name::GRAY_GLACIER => Ok(SpecId::GrayGlacier),
            edr_eth::l1::hardfork::name::MERGE => Ok(SpecId::Merge),
            edr_eth::l1::hardfork::name::SHANGHAI => Ok(SpecId::Shanghai),
            edr_eth::l1::hardfork::name::CANCUN => Ok(SpecId::Cancun),
            edr_eth::l1::hardfork::name::PRAGUE => Ok(SpecId::Prague),
            _ => Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("The provided hardfork `{s}` is not supported."),
            )),
        }
    }
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
            SpecId::Prague => edr_eth::l1::SpecId::PRAGUE,
        }
    }
}

/// Tries to parse the provided string to create a [`SpecId`] instance.
///
/// Returns an error if the string does not match any known hardfork.
#[napi(catch_unwind)]
pub fn l1_hardfork_from_string(hardfork: String) -> napi::Result<SpecId> {
    hardfork.parse()
}

#[napi(catch_unwind)]
pub fn l1_hardfork_to_string(harfork: SpecId) -> &'static str {
    match harfork {
        SpecId::Frontier => edr_eth::l1::hardfork::name::FRONTIER,
        SpecId::FrontierThawing => edr_eth::l1::hardfork::name::FRONTIER_THAWING,
        SpecId::Homestead => edr_eth::l1::hardfork::name::HOMESTEAD,
        SpecId::DaoFork => edr_eth::l1::hardfork::name::DAO_FORK,
        SpecId::Tangerine => edr_eth::l1::hardfork::name::TANGERINE,
        SpecId::SpuriousDragon => edr_eth::l1::hardfork::name::SPURIOUS_DRAGON,
        SpecId::Byzantium => edr_eth::l1::hardfork::name::BYZANTIUM,
        SpecId::Constantinople => edr_eth::l1::hardfork::name::CONSTANTINOPLE,
        SpecId::Petersburg => edr_eth::l1::hardfork::name::PETERSBURG,
        SpecId::Istanbul => edr_eth::l1::hardfork::name::ISTANBUL,
        SpecId::MuirGlacier => edr_eth::l1::hardfork::name::MUIR_GLACIER,
        SpecId::Berlin => edr_eth::l1::hardfork::name::BERLIN,
        SpecId::London => edr_eth::l1::hardfork::name::LONDON,
        SpecId::ArrowGlacier => edr_eth::l1::hardfork::name::ARROW_GLACIER,
        SpecId::GrayGlacier => edr_eth::l1::hardfork::name::GRAY_GLACIER,
        SpecId::Merge => edr_eth::l1::hardfork::name::MERGE,
        SpecId::Shanghai => edr_eth::l1::hardfork::name::SHANGHAI,
        SpecId::Cancun => edr_eth::l1::hardfork::name::CANCUN,
        SpecId::Prague => edr_eth::l1::hardfork::name::PRAGUE,
    }
}

/// Returns the latest supported OP hardfork.
///
/// The returned value will be updated after each network upgrade.
#[napi]
pub fn l1_hardfork_latest() -> SpecId {
    SpecId::Prague
}

macro_rules! export_spec_id {
    ($($variant:ident),*) => {
        $(
            #[napi]
            pub const $variant: &str = edr_eth::l1::hardfork::name::$variant;
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
    PRAGUE
);
