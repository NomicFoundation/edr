mod factory;

use std::sync::Arc;

use edr_napi::provider::ProviderFactory;
use edr_napi_core::{provider::SyncProviderFactory, spec::SyncNapiSpec};
use edr_optimism::OptimismChainSpec;
use factory::OptimismProviderFactory;
use napi_derive::napi;

#[napi]
pub const CHAIN_TYPE: &str = OptimismChainSpec::CHAIN_TYPE;

#[napi]
pub fn provider_factory() -> ProviderFactory {
    let factory: Arc<dyn SyncProviderFactory> = Arc::new(OptimismProviderFactory);
    factory.into()
}
