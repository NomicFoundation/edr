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
pub fn l1_genesis_state(hardfork: L1Hardfork) -> Vec<AccountOverride> {
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

    if hardfork < L1Hardfork::Cancun {
        Vec::new()
    } else if hardfork < L1Hardfork::Prague {
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
pub enum L1Hardfork {
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

impl FromStr for L1Hardfork {
    type Err = napi::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<edr_chain_l1::Hardfork>()
            .map(L1Hardfork::from)
            .map_err(|edr_primitives::UnknownHardfork| {
                napi::Error::new(
                    napi::Status::InvalidArg,
                    format!("The provided hardfork `{s}` is not supported."),
                )
            })
    }
}

impl From<L1Hardfork> for &'static str {
    fn from(value: L1Hardfork) -> Self {
        edr_chain_l1::Hardfork::from(value).into()
    }
}

impl From<edr_chain_l1::Hardfork> for L1Hardfork {
    fn from(value: edr_chain_l1::Hardfork) -> Self {
        match value {
            edr_chain_l1::Hardfork::Byzantium => L1Hardfork::Byzantium,
            edr_chain_l1::Hardfork::Constantinople => L1Hardfork::Constantinople,
            edr_chain_l1::Hardfork::Petersburg => L1Hardfork::Petersburg,
            edr_chain_l1::Hardfork::Istanbul => L1Hardfork::Istanbul,
            edr_chain_l1::Hardfork::MuirGlacier => L1Hardfork::MuirGlacier,
            edr_chain_l1::Hardfork::Berlin => L1Hardfork::Berlin,
            edr_chain_l1::Hardfork::London => L1Hardfork::London,
            edr_chain_l1::Hardfork::ArrowGlacier => L1Hardfork::ArrowGlacier,
            edr_chain_l1::Hardfork::GrayGlacier => L1Hardfork::GrayGlacier,
            edr_chain_l1::Hardfork::Merge => L1Hardfork::Merge,
            edr_chain_l1::Hardfork::Shanghai => L1Hardfork::Shanghai,
            edr_chain_l1::Hardfork::Cancun => L1Hardfork::Cancun,
            edr_chain_l1::Hardfork::Prague => L1Hardfork::Prague,
            edr_chain_l1::Hardfork::Osaka => L1Hardfork::Osaka,
            edr_chain_l1::Hardfork::Amsterdam => L1Hardfork::Amsterdam,
        }
    }
}

impl From<L1Hardfork> for edr_chain_l1::Hardfork {
    fn from(value: L1Hardfork) -> Self {
        match value {
            L1Hardfork::Byzantium => edr_chain_l1::Hardfork::Byzantium,
            L1Hardfork::Constantinople => edr_chain_l1::Hardfork::Constantinople,
            L1Hardfork::Petersburg => edr_chain_l1::Hardfork::Petersburg,
            L1Hardfork::Istanbul => edr_chain_l1::Hardfork::Istanbul,
            L1Hardfork::MuirGlacier => edr_chain_l1::Hardfork::MuirGlacier,
            L1Hardfork::Berlin => edr_chain_l1::Hardfork::Berlin,
            L1Hardfork::London => edr_chain_l1::Hardfork::London,
            L1Hardfork::ArrowGlacier => edr_chain_l1::Hardfork::ArrowGlacier,
            L1Hardfork::GrayGlacier => edr_chain_l1::Hardfork::GrayGlacier,
            L1Hardfork::Merge => edr_chain_l1::Hardfork::Merge,
            L1Hardfork::Shanghai => edr_chain_l1::Hardfork::Shanghai,
            L1Hardfork::Cancun => edr_chain_l1::Hardfork::Cancun,
            L1Hardfork::Prague => edr_chain_l1::Hardfork::Prague,
            L1Hardfork::Osaka => edr_chain_l1::Hardfork::Osaka,
            L1Hardfork::Amsterdam => edr_chain_l1::Hardfork::Amsterdam,
        }
    }
}

/// Tries to parse the provided string to create an [`L1Hardfork`] instance.
///
/// Returns an error if the string does not match any known hardfork.
#[napi(catch_unwind)]
pub fn l1_hardfork_from_string(hardfork: String) -> napi::Result<L1Hardfork> {
    hardfork.parse()
}

#[napi(catch_unwind)]
pub fn l1_hardfork_to_string(hardfork: L1Hardfork) -> &'static str {
    hardfork.into()
}

/// Returns the latest supported L1 hardfork.
///
/// The returned value will be updated after each network upgrade.
#[napi]
pub fn l1_hardfork_latest() -> L1Hardfork {
    L1Hardfork::Osaka
}

#[cfg(test)]
mod tests {
    use super::*;

    const VARIANTS: [L1Hardfork; 15] = [
        L1Hardfork::Byzantium,
        L1Hardfork::Constantinople,
        L1Hardfork::Petersburg,
        L1Hardfork::Istanbul,
        L1Hardfork::MuirGlacier,
        L1Hardfork::Berlin,
        L1Hardfork::London,
        L1Hardfork::ArrowGlacier,
        L1Hardfork::GrayGlacier,
        L1Hardfork::Merge,
        L1Hardfork::Shanghai,
        L1Hardfork::Cancun,
        L1Hardfork::Prague,
        L1Hardfork::Osaka,
        L1Hardfork::Amsterdam,
    ];

    /// The two hand-written `From` conversion tables must be inverses of
    /// each other.
    #[test]
    fn napi_names_parse_as_domain_hardforks() {
        for spec_id in VARIANTS {
            let name = l1_hardfork_to_string(spec_id);
            let hardfork: edr_chain_l1::Hardfork = name.parse().unwrap();
            assert_eq!(edr_chain_l1::Hardfork::from(spec_id), hardfork);
            assert_eq!(L1Hardfork::from_str(name).unwrap(), spec_id);
        }
    }
}
