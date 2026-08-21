use std::{str::FromStr, sync::Arc};

use edr_blockchain_fork::eips::{
    eip2935::{HISTORY_STORAGE_ADDRESS, HISTORY_STORAGE_UNSUPPORTED_BYTECODE},
    eip4788::{BEACON_ROOTS_ADDRESS, BEACON_ROOTS_BYTECODE},
};
use edr_chain_l1::L1ChainSpec;
use edr_napi_core::{logger::Logger, provider::SyncProvider};
use edr_provider::time::CurrentTime;
use edr_solidity::contract_decoder::ContractDecoder;
use napi::{
    bindgen_prelude::{BigInt, Uint8Array},
    tokio::runtime,
};
use napi_derive::napi;
use parking_lot::RwLock;

use crate::{
    account::AccountOverride,
    provider::{factory::SyncProviderFactory, ProviderFactory},
    subscription::{subscriber_callback_for_chain_spec, SubscriptionTsfn},
};

pub struct L1ProviderFactory;

impl SyncProviderFactory for L1ProviderFactory {
    fn create_provider(
        &self,
        runtime: runtime::Handle,
        provider_config: edr_napi_core::provider::Config,
        logger_config: edr_napi_core::logger::Config,
        subscription_callback: Arc<SubscriptionTsfn>,
        contract_decoder: Arc<RwLock<ContractDecoder>>,
    ) -> napi::Result<Arc<dyn SyncProvider>> {
        let logger =
            Logger::<L1ChainSpec, CurrentTime>::new(logger_config, Arc::clone(&contract_decoder))?;

        let provider_config =
            edr_provider::config::Provider::<edr_chain_l1::Hardfork>::try_from(provider_config)?;

        let provider = edr_provider::Provider::<L1ChainSpec>::new(
            runtime.clone(),
            Box::new(logger),
            subscriber_callback_for_chain_spec::<L1ChainSpec, CurrentTime>(subscription_callback),
            provider_config,
            contract_decoder,
            CurrentTime,
        )
        .map_err(|error| napi::Error::new(napi::Status::GenericFailure, error.to_string()))?;

        Ok(Arc::new(provider))
    }
}

#[napi]
pub const L1_CHAIN_TYPE: &str = edr_chain_l1::CHAIN_TYPE;

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
//
// N-API projection of [`edr_chain_l1::Hardfork`], which only exists to
// generate the TS enum; string conversions delegate to the domain type.
#[napi]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpecId {
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
    /// Osaka
    Osaka = 19,
    /// Amsterdam
    Amsterdam = 20,
}

impl FromStr for SpecId {
    type Err = napi::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<edr_chain_l1::Hardfork>()
            .map(SpecId::from)
            .map_err(|edr_primitives::UnknownHardfork| {
                napi::Error::new(
                    napi::Status::InvalidArg,
                    format!("The provided hardfork `{s}` is not supported."),
                )
            })
    }
}

impl From<SpecId> for &'static str {
    fn from(value: SpecId) -> Self {
        edr_chain_l1::Hardfork::from(value).into()
    }
}

impl From<edr_chain_l1::Hardfork> for SpecId {
    fn from(value: edr_chain_l1::Hardfork) -> Self {
        match value {
            edr_chain_l1::Hardfork::Byzantium => SpecId::Byzantium,
            edr_chain_l1::Hardfork::Constantinople => SpecId::Constantinople,
            edr_chain_l1::Hardfork::Petersburg => SpecId::Petersburg,
            edr_chain_l1::Hardfork::Istanbul => SpecId::Istanbul,
            edr_chain_l1::Hardfork::MuirGlacier => SpecId::MuirGlacier,
            edr_chain_l1::Hardfork::Berlin => SpecId::Berlin,
            edr_chain_l1::Hardfork::London => SpecId::London,
            edr_chain_l1::Hardfork::ArrowGlacier => SpecId::ArrowGlacier,
            edr_chain_l1::Hardfork::GrayGlacier => SpecId::GrayGlacier,
            edr_chain_l1::Hardfork::Merge => SpecId::Merge,
            edr_chain_l1::Hardfork::Shanghai => SpecId::Shanghai,
            edr_chain_l1::Hardfork::Cancun => SpecId::Cancun,
            edr_chain_l1::Hardfork::Prague => SpecId::Prague,
            edr_chain_l1::Hardfork::Osaka => SpecId::Osaka,
            edr_chain_l1::Hardfork::Amsterdam => SpecId::Amsterdam,
        }
    }
}

impl From<SpecId> for edr_chain_l1::Hardfork {
    fn from(value: SpecId) -> Self {
        match value {
            SpecId::Byzantium => edr_chain_l1::Hardfork::Byzantium,
            SpecId::Constantinople => edr_chain_l1::Hardfork::Constantinople,
            SpecId::Petersburg => edr_chain_l1::Hardfork::Petersburg,
            SpecId::Istanbul => edr_chain_l1::Hardfork::Istanbul,
            SpecId::MuirGlacier => edr_chain_l1::Hardfork::MuirGlacier,
            SpecId::Berlin => edr_chain_l1::Hardfork::Berlin,
            SpecId::London => edr_chain_l1::Hardfork::London,
            SpecId::ArrowGlacier => edr_chain_l1::Hardfork::ArrowGlacier,
            SpecId::GrayGlacier => edr_chain_l1::Hardfork::GrayGlacier,
            SpecId::Merge => edr_chain_l1::Hardfork::Merge,
            SpecId::Shanghai => edr_chain_l1::Hardfork::Shanghai,
            SpecId::Cancun => edr_chain_l1::Hardfork::Cancun,
            SpecId::Prague => edr_chain_l1::Hardfork::Prague,
            SpecId::Osaka => edr_chain_l1::Hardfork::Osaka,
            SpecId::Amsterdam => edr_chain_l1::Hardfork::Amsterdam,
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
pub fn l1_hardfork_to_string(hardfork: SpecId) -> &'static str {
    hardfork.into()
}

/// Returns the latest supported OP hardfork.
///
/// The returned value will be updated after each network upgrade.
#[napi]
pub fn l1_hardfork_latest() -> SpecId {
    SpecId::Osaka
}

#[cfg(test)]
mod tests {
    use super::*;

    const VARIANTS: [SpecId; 15] = [
        SpecId::Byzantium,
        SpecId::Constantinople,
        SpecId::Petersburg,
        SpecId::Istanbul,
        SpecId::MuirGlacier,
        SpecId::Berlin,
        SpecId::London,
        SpecId::ArrowGlacier,
        SpecId::GrayGlacier,
        SpecId::Merge,
        SpecId::Shanghai,
        SpecId::Cancun,
        SpecId::Prague,
        SpecId::Osaka,
        SpecId::Amsterdam,
    ];

    /// The two hand-written `From` conversion tables must be inverses of
    /// each other.
    #[test]
    fn napi_names_parse_as_domain_hardforks() {
        for spec_id in VARIANTS {
            let name = l1_hardfork_to_string(spec_id);
            let hardfork: edr_chain_l1::Hardfork = name.parse().unwrap();
            assert_eq!(edr_chain_l1::Hardfork::from(spec_id), hardfork);
            assert_eq!(SpecId::from_str(name).unwrap(), spec_id);
        }
    }
}
