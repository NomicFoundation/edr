use std::sync::Arc;

use edr_blockchain_fork::eips::{
    eip2935::{HISTORY_STORAGE_ADDRESS, HISTORY_STORAGE_UNSUPPORTED_BYTECODE},
    eip4788::{BEACON_ROOTS_ADDRESS, BEACON_ROOTS_BYTECODE},
};
use edr_chain_l1::{Hardfork, L1ChainSpec};
use edr_napi_core::{
    logger::Logger,
    provider::{SyncProvider, SyncProviderFactory},
    subscription::subscriber_callback_for_chain_spec,
};
use edr_provider::time::CurrentTime;
use edr_solidity::contract_decoder::ContractDecoder;
use napi::{
    bindgen_prelude::{BigInt, Uint8Array},
    tokio::runtime,
};
use napi_derive::napi;
use parking_lot::RwLock;

use crate::{account::AccountOverride, provider::ProviderFactory};

pub struct L1ProviderFactory;

impl SyncProviderFactory for L1ProviderFactory {
    fn create_provider(
        &self,
        runtime: runtime::Handle,
        provider_config: edr_napi_core::provider::Config,
        logger_config: edr_napi_core::logger::Config,
        subscription_callback: edr_napi_core::subscription::Callback,
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
pub fn l1_genesis_state(hardfork: String) -> napi::Result<Vec<AccountOverride>> {
    let hardfork = l1_hardfork_from_name(&hardfork)?;

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

    Ok(if hardfork < edr_chain_l1::Hardfork::CANCUN {
        Vec::new()
    } else if hardfork < edr_chain_l1::Hardfork::PRAGUE {
        vec![beacon_roots_account_constructor()]
    } else {
        vec![
            beacon_roots_account_constructor(),
            history_storage_account_constructor(),
        ]
    })
}

#[napi(catch_unwind)]
pub fn l1_provider_factory() -> ProviderFactory {
    let factory: Arc<dyn SyncProviderFactory> = Arc::new(L1ProviderFactory);
    factory.into()
}

/// Converts an EDR public hardfork name string to its [`Hardfork`].
/// The only place EDR's public L1 hardfork names are tied to internal variants.
fn l1_hardfork_from_name(name: &str) -> napi::Result<Hardfork> {
    Ok(match name {
        "Frontier" => Hardfork::FRONTIER,
        "Frontier Thawing" => Hardfork::FRONTIER_THAWING,
        "Homestead" => Hardfork::HOMESTEAD,
        "DAO Fork" => Hardfork::DAO_FORK,
        "Tangerine" => Hardfork::TANGERINE,
        "Spurious" => Hardfork::SPURIOUS_DRAGON,
        "Byzantium" => Hardfork::BYZANTIUM,
        "Constantinople" => Hardfork::CONSTANTINOPLE,
        "Petersburg" => Hardfork::PETERSBURG,
        "Istanbul" => Hardfork::ISTANBUL,
        "MuirGlacier" => Hardfork::MUIR_GLACIER,
        "Berlin" => Hardfork::BERLIN,
        "London" => Hardfork::LONDON,
        "Arrow Glacier" => Hardfork::ARROW_GLACIER,
        "Gray Glacier" => Hardfork::GRAY_GLACIER,
        "Merge" => Hardfork::MERGE,
        "Shanghai" => Hardfork::SHANGHAI,
        "Cancun" => Hardfork::CANCUN,
        "Prague" => Hardfork::PRAGUE,
        "Osaka" => Hardfork::OSAKA,
        _ => {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("The provided hardfork `{name}` is not supported."),
            ))
        }
    })
}
