use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use napi_derive::napi;
use parking_lot::RwLock;

use crate::config::TracingConfigWithBuffers;

/// Configuration for loading a project's compilation output from disk.
#[napi(object)]
pub struct ProjectArtifactsConfig {
    /// The path of the project's artifacts directory.
    pub artifacts_dir: String,
    /// The path of the directory containing the project's build info files.
    /// Defaults to the `build-info` subdirectory of `artifactsDir`.
    pub build_info_dir: Option<String>,
    /// Whether to ignore contracts whose name starts with "Ignored".
    pub ignore_contracts: Option<bool>,
}

#[napi]
pub struct ContractDecoder {
    inner: Arc<RwLock<edr_solidity::contract_decoder::ContractDecoder>>,
}

#[napi]
impl ContractDecoder {
    #[doc = "Creates an empty instance."]
    #[napi(constructor, catch_unwind)]
    // Following TS convention for the constructor without arguments to be `new()`.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            inner: Arc::default(),
        }
    }

    #[doc = "Creates a new instance with the provided configuration."]
    #[napi(factory, catch_unwind)]
    pub fn with_contracts(config: TracingConfigWithBuffers) -> napi::Result<Self> {
        let build_info_config = edr_solidity::artifacts::BuildInfoConfig::parse_from_buffers(
            (&edr_napi_core::solidity::config::TracingConfigWithBuffers::from(config)).into(),
        )
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;

        let contract_decoder =
            edr_solidity::contract_decoder::ContractDecoder::new(build_info_config);

        Ok(Self {
            inner: Arc::new(RwLock::new(contract_decoder)),
        })
    }

    #[doc = "Creates a new instance by reading the project's build infos from disk."]
    #[napi(factory, catch_unwind)]
    pub fn from_project(config: ProjectArtifactsConfig) -> napi::Result<Self> {
        let build_info_dir = config.build_info_dir.map_or_else(
            || edr_solidity::project::default_build_info_dir(Path::new(&config.artifacts_dir)),
            PathBuf::from,
        );

        let build_info_config =
            edr_solidity::project::load_build_info_config(&build_info_dir, config.ignore_contracts)
                .map_err(|error| napi::Error::from_reason(error.to_string()))?;

        let contract_decoder =
            edr_solidity::contract_decoder::ContractDecoder::new(build_info_config);

        Ok(Self {
            inner: Arc::new(RwLock::new(contract_decoder)),
        })
    }
}

impl ContractDecoder {
    /// Returns a reference to the inner contract decoder.
    pub fn as_inner(&self) -> &Arc<RwLock<edr_solidity::contract_decoder::ContractDecoder>> {
        &self.inner
    }
}

impl From<Arc<RwLock<edr_solidity::contract_decoder::ContractDecoder>>> for ContractDecoder {
    fn from(
        contract_decoder: Arc<RwLock<edr_solidity::contract_decoder::ContractDecoder>>,
    ) -> Self {
        Self {
            inner: contract_decoder,
        }
    }
}
