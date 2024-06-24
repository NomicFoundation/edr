use std::{collections::HashMap, path::PathBuf};

use alloy_primitives::{address, Address};
use edr_eth::U256;
use forge::{
    inspectors::cheatcodes::CheatsConfigOptions,
    opts::{Env as EvmEnv, EvmOpts},
};
use foundry_compilers::ProjectPathsConfig;
use foundry_config::{
    cache::StorageCachingConfig, fs_permissions::PathPermission, FsPermissions, FuzzConfig,
    GasLimit, InvariantConfig, RpcEndpoint, RpcEndpoints,
};

/// Solidity tests configuration
#[derive(Clone, Debug)]
pub(super) struct SolidityTestsConfig {
    /// Project paths configuration
    pub project_paths_config: ProjectPathsConfig,
    /// Cheats configuration options
    pub cheats_config_options: CheatsConfigOptions,
    /// EVM options
    pub evm_opts: EvmOpts,
    /// Configuration for fuzz testing
    pub fuzz: FuzzConfig,
    /// Configuration for invariant testing
    pub invariant: InvariantConfig,
}

/// Default address for tx.origin for Foundry
///
/// `0x1804c8AB1F12E6bbf3894d4083f33e07309d1f38`
const DEFAULT_SENDER: Address = address!("1804c8AB1F12E6bbf3894d4083f33e07309d1f38");

impl SolidityTestsConfig {
    /// Create a new `SolidityTestsConfig` instance
    pub fn new(gas_report: bool) -> Self {
        // Matches Foundry config defaults
        let gas_limit: GasLimit = i64::MAX.into();
        let evm_opts = EvmOpts {
            env: EvmEnv {
                gas_limit: gas_limit.into(),
                chain_id: None,
                tx_origin: DEFAULT_SENDER,
                block_number: 1,
                block_timestamp: 1,
                ..Default::default()
            },
            sender: DEFAULT_SENDER,
            initial_balance: U256::from(0xffffffffffffffffffffffffu128),
            ffi: false,
            verbosity: 0,
            memory_limit: 1 << 27, // 2**27 = 128MiB = 134_217_728 bytes
            ..EvmOpts::default()
        };

        // Matches Foundry config defaults
        let mut fuzz = FuzzConfig::new("cache/fuzz".into());
        let mut invariant = InvariantConfig::new("cache/invariant".into());
        if !gas_report {
            fuzz.gas_report_samples = 0;
            invariant.gas_report_samples = 0;
        }
        let project_root = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/foundry/testdata"
        ));

        // TODO https://github.com/NomicFoundation/edr/issues/487
        let project_paths_config = ProjectPathsConfig::builder().build_with_root(project_root);

        let artifacts: PathBuf = project_paths_config
            .artifacts
            .file_name()
            .expect("artifacts are not relative")
            .into();

        // Matches Foundry config defaults
        let cheats_config_options = CheatsConfigOptions {
            rpc_endpoints: RpcEndpoints::new([(
                "alchemy",
                RpcEndpoint::Url("${ALCHEMY_URL}".to_string()),
            )]),
            unchecked_cheatcode_artifacts: false,
            prompt_timeout: 0,
            rpc_storage_caching: StorageCachingConfig::default(),
            fs_permissions: FsPermissions::new([PathPermission::read(artifacts)]),
            labels: HashMap::default(),
        };

        Self {
            project_paths_config,
            cheats_config_options,
            evm_opts,
            fuzz,
            invariant,
        }
    }
}
