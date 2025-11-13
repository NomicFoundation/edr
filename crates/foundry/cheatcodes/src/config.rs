use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use alloy_primitives::U256;
use alloy_primitives::map::AddressHashMap;
use edr_common::fs::normalize_path;
use edr_solidity::artifacts::ArtifactId;
use foundry_compilers::utils::canonicalize;
use foundry_evm_core::{contracts::ContractsByArtifact, evm_context::HardforkTr, opts::EvmOpts};

use super::{FsAccessKind, FsPermissions, Result, RpcEndpoint, RpcEndpointUrl, RpcEndpoints};
use crate::{cache::StorageCachingConfig, Vm::Rpc};

/// Additional, configurable context the `Cheatcodes` inspector has access to
///
/// This is essentially a subset of various `Config` settings `Cheatcodes` needs to know.
#[derive(Clone, Debug)]
pub struct CheatsConfig<HardforkT> {
    /// Whether the execution is in the context of a test run, gas snapshot or
    /// code coverage.
    pub execution_context: ExecutionContextConfig,
    /// Whether the FFI cheatcode is enabled.
    pub ffi: bool,
    /// Sets a timeout for vm.prompt cheatcodes
    pub prompt_timeout: Duration,
    /// Optional RPC cache path. If this is none, then no RPC calls will be
    /// cached, otherwise data is cached to `<rpc_cache_path>/<chain
    /// id>/<block number>`. Caching can be disabled for specific chains
    /// with `rpc_storage_caching`.
    pub rpc_cache_path: Option<PathBuf>,
    /// RPC storage caching settings determines what chains and endpoints to cache
    pub rpc_storage_caching: StorageCachingConfig,
    /// All known endpoints and their aliases
    pub rpc_endpoints: RpcEndpoints,
    /// Filesystem permissions for cheatcodes like `writeFile`, `readFile`
    pub fs_permissions: FsPermissions,
    /// Project root
    pub project_root: PathBuf,
    /// How the evm was configured by the user
    pub evm_opts: EvmOpts<HardforkT>,
    /// Address labels from config
    pub labels: AddressHashMap<String>,
    /// Solidity compilation artifacts.
    pub available_artifacts: Arc<ContractsByArtifact>,
    /// Currently running artifact.
    pub running_artifact: Option<ArtifactId>,
    /// Optional seed for the RNG algorithm.
    pub seed: Option<U256>,
    /// Whether to allow `expectRevert` to work for internal calls.
    pub internal_expect_revert: bool,
    /// Mapping of chain aliases to chain data
    pub chains: HashMap<String, ChainData>,
    /// Mapping of chain IDs to their aliases
    pub chain_id_to_alias: HashMap<u64, String>,
}

/// Chain data for getChain cheatcodes
#[derive(Clone, Debug)]
pub struct ChainData {
    pub name: String,
    pub chain_id: u64,
    pub default_rpc_url: String, // Store default RPC URL
}

/// Solidity test execution contexts.
#[derive(Clone, Debug, Default)]
pub enum ExecutionContextConfig {
    /// Test execution context.
    Test,
    /// Code coverage execution context.
    Coverage,
    /// Gas snapshot execution context.
    Snapshot,
    /// Unknown execution context.
    #[default]
    Unknown,
}

/// Configuration options specific to cheat codes.
#[derive(Clone, Debug, Default)]
pub struct CheatsConfigOptions {
    /// Solidity test execution contexts.
    pub execution_context: ExecutionContextConfig,
    /// Multiple rpc endpoints and their aliases
    pub rpc_endpoints: RpcEndpoints,
    /// Optional RPC cache path. If this is none, then no RPC calls will be
    /// cached, otherwise data is cached to `<rpc_cache_path>/<chain
    /// id>/<block number>`. Caching can be disabled for specific chains
    /// with `rpc_storage_caching`.
    pub rpc_cache_path: Option<PathBuf>,
    /// RPC storage caching settings determines what chains and endpoints to
    /// cache
    pub rpc_storage_caching: StorageCachingConfig,
    /// Configures the permissions of cheat codes that touch the file system.
    ///
    /// This includes what operations can be executed (read, write)
    pub fs_permissions: FsPermissions,
    /// Sets a timeout in seconds for vm.prompt cheatcodes
    pub prompt_timeout: u64,
    /// Address labels
    pub labels: AddressHashMap<String>,
    /// Optional seed for the RNG algorithm.
    pub seed: Option<U256>,
    /// Allow expecting reverts with `expectRevert` at the same callstack depth
    /// as the test.
    pub allow_internal_expect_revert: bool,
}

impl<HardforkT: HardforkTr> CheatsConfig<HardforkT> {
    /// Extracts the necessary settings from the Config
    pub fn new(
        project_root: PathBuf,
        config: CheatsConfigOptions,
        evm_opts: EvmOpts<HardforkT>,
        available_artifacts: Arc<ContractsByArtifact>,
        running_artifact: Option<ArtifactId>,
    ) -> Self {
        let CheatsConfigOptions {
            execution_context,
            rpc_endpoints,
            rpc_cache_path,
            prompt_timeout,
            rpc_storage_caching,
            fs_permissions,
            labels,
            seed, allow_internal_expect_revert,
        } = config;

        // TODO
        // let mut allowed_paths = vec![config.root.clone()];
        // allowed_paths.extend(config.libs.iter().cloned());
        // allowed_paths.extend(config.allow_paths.iter().cloned());
        let fs_permissions = fs_permissions.joined(&project_root);

        Self {
            execution_context,
            ffi: evm_opts.ffi,
            prompt_timeout: Duration::from_secs(prompt_timeout),
            rpc_cache_path,
            rpc_storage_caching,
            rpc_endpoints,
            fs_permissions,
            project_root,
            evm_opts,
            labels,
            available_artifacts,
            running_artifact,
            seed,
            internal_expect_revert: allow_internal_expect_revert,
            chains: HashMap::new(),
            chain_id_to_alias: HashMap::new(),
        }
    }

    /// Attempts to canonicalize (see [`std::fs::canonicalize`]) the path.
    ///
    /// Canonicalization fails for non-existing paths, in which case we just normalize the path.
    pub fn normalized_path(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = self.project_root.join(path);
        canonicalize(&path).unwrap_or_else(|_| normalize_path(&path))
    }

    /// Returns true if the given path is allowed, if any path `allowed_paths` is an ancestor of the
    /// path
    ///
    /// We only allow paths that are inside  allowed paths. To prevent path traversal
    /// ("../../etc/passwd") we canonicalize/normalize the path first. We always join with the
    /// configured root directory.
    pub fn is_path_allowed(&self, path: impl AsRef<Path>, kind: FsAccessKind) -> bool {
        self.is_normalized_path_allowed(&self.normalized_path(path), kind)
    }

    fn is_normalized_path_allowed(&self, path: &Path, kind: FsAccessKind) -> bool {
        self.fs_permissions.is_path_allowed(path, kind)
    }

    /// Returns an error if no access is granted to access `path`, See also [`Self::is_path_allowed`]
    ///
    /// Returns the normalized version of `path`, see [`CheatsConfig::normalized_path`]
    pub fn ensure_path_allowed(
        &self,
        path: impl AsRef<Path>,
        kind: FsAccessKind,
    ) -> Result<PathBuf> {
        let path = path.as_ref();
        let normalized = self.normalized_path(path);
        ensure!(
            self.is_normalized_path_allowed(&normalized, kind),
            "the path {} is not allowed to be accessed for {kind} operations",
            normalized.strip_prefix(&self.project_root).unwrap_or(path).display()
        );
        Ok(normalized)
    }

    /// Returns the RPC to use
    ///
    /// If `url_or_alias` is a known alias in the `ResolvedRpcEndpoints` then it returns the
    /// corresponding URL of that alias. otherwise this assumes `url_or_alias` is itself a URL
    /// if it starts with a `http` or `ws` scheme.
    ///
    /// If the url is a path to an existing file, it is also considered a valid RPC URL, IPC path.
    ///
    /// # Errors
    ///
    ///  - Returns an error if `url_or_alias` is a known alias but references an unresolved env var.
    ///  - Returns an error if `url_or_alias` is not an alias but does not start with a `http` or
    ///    `ws` `scheme` and is not a path to an existing file
    pub fn rpc_endpoint(&self, url_or_alias: &str) -> Result<RpcEndpoint> {
        if let Some(endpoint) = self.rpc_endpoints.get(url_or_alias) {
            Ok(endpoint.clone())
        } else {
            // check if it's a URL or a path to an existing file to an ipc socket
            if url_or_alias.starts_with("http") ||
                url_or_alias.starts_with("ws") ||
                // check for existing ipc file
                Path::new(url_or_alias).exists()
            {
                let url = RpcEndpointUrl::new(url_or_alias);
                Ok(RpcEndpoint::new(url))
            } else {
                Err(fmt_err!("invalid rpc url: {url_or_alias}"))
            }
        }
    }

    /// Returns all the RPC urls and their alias.
    pub fn rpc_urls(&self) -> Result<Vec<Rpc>> {
        let mut urls = Vec::with_capacity(self.rpc_endpoints.len());
        for alias in self.rpc_endpoints.keys() {
            let url = self.rpc_endpoint(alias)?.url;
            urls.push(Rpc { key: alias.clone(), url: url.into() });
        }
        Ok(urls)
    }

    /// Initialize default chain data (similar to initializeStdChains in Solidity)
    pub fn initialize_chain_data(&mut self) {
        if !self.chains.is_empty() {
            return; // Already initialized
        }

        // Use the same function to create chains
        let chains = create_default_chains();

        // Add all chains to the config
        for (alias, data) in chains {
            self.set_chain_with_default_rpc_url(&alias, data);
        }
    }

    /// Set chain with default RPC URL (similar to setChainWithDefaultRpcUrl in Solidity)
    pub fn set_chain_with_default_rpc_url(&mut self, alias: &str, data: ChainData) {
        // Store the default RPC URL is already stored in the data
        // No need to clone it separately

        // Add chain data
        self.set_chain_data(alias, data);
    }

    /// Set chain data for a specific alias
    pub fn set_chain_data(&mut self, alias: &str, data: ChainData) {
        // Remove old chain ID mapping if it exists
        if let Some(old_data) = self.chains.get(alias) {
            self.chain_id_to_alias.remove(&old_data.chain_id);
        }

        // Add new mappings
        self.chain_id_to_alias.insert(data.chain_id, alias.to_string());
        self.chains.insert(alias.to_string(), data);
    }

    /// Get chain data by alias
    pub fn get_chain_data_by_alias_non_mut(&self, alias: &str) -> Result<ChainData> {
        // Initialize chains if not already done
        if self.chains.is_empty() {
            // Create a temporary copy with initialized chains
            // This is inefficient but handles the edge case
            let temp_chains = create_default_chains();

            if let Some(data) = temp_chains.get(alias) {
                return Ok(data.clone());
            }
        } else {
            // Normal path - chains are initialized
            if let Some(data) = self.chains.get(alias) {
                return Ok(data.clone());
            }
        }

        // Chain not found in either case
        Err(fmt_err!("vm.getChain: Chain with alias \"{}\" not found", alias))
    }

    /// Get RPC URL for an alias
    pub fn get_rpc_url_non_mut(&self, alias: &str) -> Result<String> {
        // Try to get from config first
        if let Ok(endpoint) = self.rpc_endpoint(alias) { Ok(endpoint.url.into()) } else {
            // If not in config, try to get default URL
            let chain_data = self.get_chain_data_by_alias_non_mut(alias)?;
            Ok(chain_data.default_rpc_url)
        }
    }
}

impl<HardforkT: HardforkTr> Default for CheatsConfig<HardforkT> {
    fn default() -> Self {
        Self {
            execution_context: ExecutionContextConfig::default(),
            ffi: false,
            prompt_timeout: Duration::from_secs(120),
            rpc_cache_path: None,
            rpc_storage_caching: StorageCachingConfig::default(),
            rpc_endpoints: RpcEndpoints::default(),
            fs_permissions: FsPermissions::default(),
            project_root: PathBuf::default(),
            evm_opts: EvmOpts::default(),
            labels: AddressHashMap::<String>::default(),
            available_artifacts: Arc::<ContractsByArtifact>::default(),
            running_artifact: None,
            seed: None,
            internal_expect_revert: false,
            chains: HashMap::new(),
            chain_id_to_alias: HashMap::new(),
        }
    }
}

// Helper function to set default chains
fn create_default_chains() -> HashMap<String, ChainData> {
    let mut chains = HashMap::new();

    // Define all chains in one place
    chains.insert(
        "anvil".to_string(),
        ChainData {
            name: "Anvil".to_string(),
            chain_id: 31337,
            default_rpc_url: "http://127.0.0.1:8545".to_string(),
        },
    );

    chains.insert(
        "mainnet".to_string(),
        ChainData {
            name: "Mainnet".to_string(),
            chain_id: 1,
            default_rpc_url: "https://eth.llamarpc.com".to_string(),
        },
    );

    chains.insert(
        "sepolia".to_string(),
        ChainData {
            name: "Sepolia".to_string(),
            chain_id: 11155111,
            default_rpc_url: "https://sepolia.infura.io/v3/b9794ad1ddf84dfb8c34d6bb5dca2001"
                .to_string(),
        },
    );

    chains.insert(
        "holesky".to_string(),
        ChainData {
            name: "Holesky".to_string(),
            chain_id: 17000,
            default_rpc_url: "https://rpc.holesky.ethpandaops.io".to_string(),
        },
    );

    chains.insert(
        "optimism".to_string(),
        ChainData {
            name: "Optimism".to_string(),
            chain_id: 10,
            default_rpc_url: "https://mainnet.optimism.io".to_string(),
        },
    );

    chains.insert(
        "optimism_sepolia".to_string(),
        ChainData {
            name: "Optimism Sepolia".to_string(),
            chain_id: 11155420,
            default_rpc_url: "https://sepolia.optimism.io".to_string(),
        },
    );

    chains.insert(
        "arbitrum_one".to_string(),
        ChainData {
            name: "Arbitrum One".to_string(),
            chain_id: 42161,
            default_rpc_url: "https://arb1.arbitrum.io/rpc".to_string(),
        },
    );

    chains.insert(
        "arbitrum_one_sepolia".to_string(),
        ChainData {
            name: "Arbitrum One Sepolia".to_string(),
            chain_id: 421614,
            default_rpc_url: "https://sepolia-rollup.arbitrum.io/rpc".to_string(),
        },
    );

    chains.insert(
        "arbitrum_nova".to_string(),
        ChainData {
            name: "Arbitrum Nova".to_string(),
            chain_id: 42170,
            default_rpc_url: "https://nova.arbitrum.io/rpc".to_string(),
        },
    );

    chains.insert(
        "polygon".to_string(),
        ChainData {
            name: "Polygon".to_string(),
            chain_id: 137,
            default_rpc_url: "https://polygon-rpc.com".to_string(),
        },
    );

    chains.insert(
        "polygon_amoy".to_string(),
        ChainData {
            name: "Polygon Amoy".to_string(),
            chain_id: 80002,
            default_rpc_url: "https://rpc-amoy.polygon.technology".to_string(),
        },
    );

    chains.insert(
        "avalanche".to_string(),
        ChainData {
            name: "Avalanche".to_string(),
            chain_id: 43114,
            default_rpc_url: "https://api.avax.network/ext/bc/C/rpc".to_string(),
        },
    );

    chains.insert(
        "avalanche_fuji".to_string(),
        ChainData {
            name: "Avalanche Fuji".to_string(),
            chain_id: 43113,
            default_rpc_url: "https://api.avax-test.network/ext/bc/C/rpc".to_string(),
        },
    );

    chains.insert(
        "bnb_smart_chain".to_string(),
        ChainData {
            name: "BNB Smart Chain".to_string(),
            chain_id: 56,
            default_rpc_url: "https://bsc-dataseed1.binance.org".to_string(),
        },
    );

    chains.insert(
        "bnb_smart_chain_testnet".to_string(),
        ChainData {
            name: "BNB Smart Chain Testnet".to_string(),
            chain_id: 97,
            default_rpc_url: "https://rpc.ankr.com/bsc_testnet_chapel".to_string(),
        },
    );

    chains.insert(
        "gnosis_chain".to_string(),
        ChainData {
            name: "Gnosis Chain".to_string(),
            chain_id: 100,
            default_rpc_url: "https://rpc.gnosischain.com".to_string(),
        },
    );

    chains.insert(
        "moonbeam".to_string(),
        ChainData {
            name: "Moonbeam".to_string(),
            chain_id: 1284,
            default_rpc_url: "https://rpc.api.moonbeam.network".to_string(),
        },
    );

    chains.insert(
        "moonriver".to_string(),
        ChainData {
            name: "Moonriver".to_string(),
            chain_id: 1285,
            default_rpc_url: "https://rpc.api.moonriver.moonbeam.network".to_string(),
        },
    );

    chains.insert(
        "moonbase".to_string(),
        ChainData {
            name: "Moonbase".to_string(),
            chain_id: 1287,
            default_rpc_url: "https://rpc.testnet.moonbeam.network".to_string(),
        },
    );

    chains.insert(
        "base_sepolia".to_string(),
        ChainData {
            name: "Base Sepolia".to_string(),
            chain_id: 84532,
            default_rpc_url: "https://sepolia.base.org".to_string(),
        },
    );

    chains.insert(
        "base".to_string(),
        ChainData {
            name: "Base".to_string(),
            chain_id: 8453,
            default_rpc_url: "https://mainnet.base.org".to_string(),
        },
    );

    chains.insert(
        "blast_sepolia".to_string(),
        ChainData {
            name: "Blast Sepolia".to_string(),
            chain_id: 168587773,
            default_rpc_url: "https://sepolia.blast.io".to_string(),
        },
    );

    chains.insert(
        "blast".to_string(),
        ChainData {
            name: "Blast".to_string(),
            chain_id: 81457,
            default_rpc_url: "https://rpc.blast.io".to_string(),
        },
    );

    chains.insert(
        "fantom_opera".to_string(),
        ChainData {
            name: "Fantom Opera".to_string(),
            chain_id: 250,
            default_rpc_url: "https://rpc.ankr.com/fantom/".to_string(),
        },
    );

    chains.insert(
        "fantom_opera_testnet".to_string(),
        ChainData {
            name: "Fantom Opera Testnet".to_string(),
            chain_id: 4002,
            default_rpc_url: "https://rpc.ankr.com/fantom_testnet/".to_string(),
        },
    );

    chains.insert(
        "fraxtal".to_string(),
        ChainData {
            name: "Fraxtal".to_string(),
            chain_id: 252,
            default_rpc_url: "https://rpc.frax.com".to_string(),
        },
    );

    chains.insert(
        "fraxtal_testnet".to_string(),
        ChainData {
            name: "Fraxtal Testnet".to_string(),
            chain_id: 2522,
            default_rpc_url: "https://rpc.testnet.frax.com".to_string(),
        },
    );

    chains.insert(
        "berachain_bartio_testnet".to_string(),
        ChainData {
            name: "Berachain bArtio Testnet".to_string(),
            chain_id: 80084,
            default_rpc_url: "https://bartio.rpc.berachain.com".to_string(),
        },
    );

    chains.insert(
        "flare".to_string(),
        ChainData {
            name: "Flare".to_string(),
            chain_id: 14,
            default_rpc_url: "https://flare-api.flare.network/ext/C/rpc".to_string(),
        },
    );

    chains.insert(
        "flare_coston2".to_string(),
        ChainData {
            name: "Flare Coston2".to_string(),
            chain_id: 114,
            default_rpc_url: "https://coston2-api.flare.network/ext/C/rpc".to_string(),
        },
    );

    chains.insert(
        "mode".to_string(),
        ChainData {
            name: "Mode".to_string(),
            chain_id: 34443,
            default_rpc_url: "https://mode.drpc.org".to_string(),
        },
    );

    chains.insert(
        "mode_sepolia".to_string(),
        ChainData {
            name: "Mode Sepolia".to_string(),
            chain_id: 919,
            default_rpc_url: "https://sepolia.mode.network".to_string(),
        },
    );

    chains.insert(
        "zora".to_string(),
        ChainData {
            name: "Zora".to_string(),
            chain_id: 7777777,
            default_rpc_url: "https://zora.drpc.org".to_string(),
        },
    );

    chains.insert(
        "zora_sepolia".to_string(),
        ChainData {
            name: "Zora Sepolia".to_string(),
            chain_id: 999999999,
            default_rpc_url: "https://sepolia.rpc.zora.energy".to_string(),
        },
    );

    chains.insert(
        "race".to_string(),
        ChainData {
            name: "Race".to_string(),
            chain_id: 6805,
            default_rpc_url: "https://racemainnet.io".to_string(),
        },
    );

    chains.insert(
        "race_sepolia".to_string(),
        ChainData {
            name: "Race Sepolia".to_string(),
            chain_id: 6806,
            default_rpc_url: "https://racemainnet.io".to_string(),
        },
    );

    chains.insert(
        "metal".to_string(),
        ChainData {
            name: "Metal".to_string(),
            chain_id: 1750,
            default_rpc_url: "https://metall2.drpc.org".to_string(),
        },
    );

    chains.insert(
        "metal_sepolia".to_string(),
        ChainData {
            name: "Metal Sepolia".to_string(),
            chain_id: 1740,
            default_rpc_url: "https://testnet.rpc.metall2.com".to_string(),
        },
    );

    chains.insert(
        "binary".to_string(),
        ChainData {
            name: "Binary".to_string(),
            chain_id: 624,
            default_rpc_url: "https://rpc.zero.thebinaryholdings.com".to_string(),
        },
    );

    chains.insert(
        "binary_sepolia".to_string(),
        ChainData {
            name: "Binary Sepolia".to_string(),
            chain_id: 625,
            default_rpc_url: "https://rpc.zero.thebinaryholdings.com".to_string(),
        },
    );

    chains.insert(
        "orderly".to_string(),
        ChainData {
            name: "Orderly".to_string(),
            chain_id: 291,
            default_rpc_url: "https://rpc.orderly.network".to_string(),
        },
    );

    chains.insert(
        "orderly_sepolia".to_string(),
        ChainData {
            name: "Orderly Sepolia".to_string(),
            chain_id: 4460,
            default_rpc_url: "https://testnet-rpc.orderly.org".to_string(),
        },
    );

    chains
}

#[cfg(test)]
mod tests {
    use revm::primitives::hardfork::SpecId;

    use super::*;
    use crate::{cache::StorageCachingConfig, PathPermission};

    fn config(root: &str, fs_permissions: FsPermissions) -> CheatsConfig<SpecId> {
        let cheats_config_options = CheatsConfigOptions {
            execution_context: ExecutionContextConfig::default(),
            rpc_endpoints: RpcEndpoints::default(),
            rpc_cache_path: None,
            rpc_storage_caching: StorageCachingConfig::default(),
            fs_permissions,
            prompt_timeout: 0,
            labels: AddressHashMap::<String>::default(),
            seed: None,
            allow_internal_expect_revert: false,
        };

        CheatsConfig::new(
            PathBuf::from(root),
            cheats_config_options,
            EvmOpts::default(),
            Arc::<ContractsByArtifact>::default(),
            None,
        )
    }

    #[test]
    fn test_allowed_paths() {
        fn test_cases(config: CheatsConfig<SpecId>) {
            assert!(config
                .ensure_path_allowed("./t.txt", FsAccessKind::Read)
                .is_ok());
            assert!(config
                .ensure_path_allowed("./t.txt", FsAccessKind::Write)
                .is_ok());
            assert!(config
                .ensure_path_allowed("../root/t.txt", FsAccessKind::Read)
                .is_ok());
            assert!(config
                .ensure_path_allowed("../root/t.txt", FsAccessKind::Write)
                .is_ok());
            assert!(config
                .ensure_path_allowed("../../root/t.txt", FsAccessKind::Read)
                .is_err());
            assert!(config
                .ensure_path_allowed("../../root/t.txt", FsAccessKind::Write)
                .is_err());

            assert!(config
                .ensure_path_allowed("/my/project/root/t.txt", FsAccessKind::Read)
                .is_ok());
            assert!(config
                .ensure_path_allowed("/my/project/root/../root/t.txt", FsAccessKind::Write)
                .is_ok());

            assert!(config
                .ensure_path_allowed("/other/project/root/t.txt", FsAccessKind::Read)
                .is_err());
        }

        let root = "/my/project/root/";

        test_cases(config(
            root,
            FsPermissions::new(vec![PathPermission::read_write_directory("./")]),
        ));

        test_cases(config(
            root,
            FsPermissions::new(vec![PathPermission::read_write_directory(
                "/my/project/root",
            )]),
        ));
    }

    #[test]
    fn file_permissions() {
        let permissions = FsPermissions::new(vec![
            PathPermission::read_file("./out/contracts/ReadContract.sol"),
            PathPermission::read_write_file("./out/contracts/ReadWriteContract.sol"),
        ]);

        let root = "/my/project/root/";
        let config = config(root, permissions);

        assert!(config
            .ensure_path_allowed("./out/contracts/ReadContract.sol", FsAccessKind::Read)
            .is_ok());
        assert!(config
            .ensure_path_allowed("./out/contracts/ReadWriteContract.sol", FsAccessKind::Write)
            .is_ok());
        assert!(
            config
                .ensure_path_allowed(
                    "./out/contracts/NoPermissionContract.sol",
                    FsAccessKind::Read
                )
                .is_err()
                && config
                    .ensure_path_allowed(
                        "./out/contracts/NoPermissionContract.sol",
                        FsAccessKind::Write
                    )
                    .is_err()
        );
    }

    #[test]
    fn directory_permissions() {
        let permissions = FsPermissions::new(vec![
            PathPermission::read_directory("./out/contracts"),
            PathPermission::read_write_directory("./out/contracts/readwrite/"),
        ]);

        let root = "/my/project/root/";
        let config = config(root, permissions);

        assert!(config
            .ensure_path_allowed("./out/contracts", FsAccessKind::Read)
            .is_ok());
        assert!(config
            .ensure_path_allowed("./out/contracts", FsAccessKind::Write)
            .is_err());

        assert!(config
            .ensure_path_allowed("./out/contracts/readwrite", FsAccessKind::Read)
            .is_ok());
        assert!(config
            .ensure_path_allowed("./out/contracts/readwrite", FsAccessKind::Write)
            .is_ok());

        assert!(config
            .ensure_path_allowed("./out", FsAccessKind::Read)
            .is_err());
        assert!(config
            .ensure_path_allowed("./out", FsAccessKind::Write)
            .is_err());
    }

    #[test]
    fn file_and_directory_permissions() {
        let permissions = FsPermissions::new(vec![
            PathPermission::read_directory("./out"),
            PathPermission::write_file("./out/WriteContract.sol"),
        ]);

        let root = "/my/project/root/";
        let config = config(root, permissions);

        assert!(config
            .ensure_path_allowed("./out", FsAccessKind::Read)
            .is_ok());
        assert!(config
            .ensure_path_allowed("./out/WriteContract.sol", FsAccessKind::Write)
            .is_ok());

        // Inherited read from directory
        assert!(config
            .ensure_path_allowed("./out/ReadContract.sol", FsAccessKind::Read)
            .is_ok());
        // No permission for writing
        assert!(config
            .ensure_path_allowed("./out/ReadContract.sol", FsAccessKind::Write)
            .is_err());
    }

    #[test]
    fn nested_permissions() {
        let permissions = FsPermissions::new(vec![
            PathPermission::read_directory("./"),
            PathPermission::write_directory("./out"),
            PathPermission::read_write_directory("./out/contracts"),
        ]);

        let root = "/my/project/root/";
        let config = config(root, permissions);

        assert!(config
            .ensure_path_allowed("./out/contracts/MyContract.sol", FsAccessKind::Write)
            .is_ok());

        assert!(config
            .ensure_path_allowed("./out/contracts/MyContract.sol", FsAccessKind::Read)
            .is_ok());
        assert!(config
            .ensure_path_allowed("./out/MyContract.sol", FsAccessKind::Write)
            .is_ok());
        assert!(config
            .ensure_path_allowed("./out/MyContract.sol", FsAccessKind::Read)
            .is_err());
    }

    #[test]
    fn exclude_file() {
        let permissions = FsPermissions::new(vec![
            PathPermission::read_write_directory("./out"),
            PathPermission::none("./out/Config.toml"),
        ]);

        let root = "/my/project/root/";
        let config = config(root, permissions);

        assert!(config
            .ensure_path_allowed("./out/Config.toml", FsAccessKind::Read)
            .is_err());
        assert!(config
            .ensure_path_allowed("./out/Config.toml", FsAccessKind::Write)
            .is_err());
        assert!(config
            .ensure_path_allowed("./out/OtherFile.sol", FsAccessKind::Read)
            .is_ok());
        assert!(config
            .ensure_path_allowed("./out/OtherFile.sol", FsAccessKind::Write)
            .is_ok());
    }
}
