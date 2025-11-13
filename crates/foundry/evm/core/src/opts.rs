use super::fork::{environment, provider::ProviderBuilder};
use crate::{evm_context::BlockEnvMut, fork::configure_env};
use alloy_chains::Chain;
use alloy_primitives::{Address, B256, U256};
use alloy_provider::{network::AnyRpcBlock, Provider};
use edr_defaults::ALCHEMY_FREE_TIER_CUPS;
use eyre::WrapErr;
use op_revm::{transaction::deposit::DepositTransactionParts, OpTransaction};
use revm::{
    context::{BlockEnv, TxEnv},
    context_interface::Block,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Write;
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvmOpts<HardforkT> {
    /// The EVM environment configuration.
    #[serde(flatten)]
    pub env: Env,

    /// The hardfork to use for the EVM.
    pub spec: HardforkT,

    /// Fetch state over a remote instead of starting from empty state.
    #[serde(rename = "eth_rpc_url")]
    pub fork_url: Option<String>,

    /// Pins the block number for the state fork.
    pub fork_block_number: Option<u64>,

    /// The number of retries.
    pub fork_retries: Option<u32>,

    /// Initial retry backoff.
    pub fork_retry_backoff: Option<u64>,

    /// Headers to use with `fork_url`
    pub fork_headers: Option<Vec<String>>,

    /// The available compute units per second.
    ///
    /// See also <https://docs.alchemy.com/reference/compute-units#what-are-cups-compute-units-per-second>
    pub compute_units_per_second: Option<u64>,

    /// Disables RPC rate limiting entirely.
    pub no_rpc_rate_limit: bool,

    /// The initial balance of each deployed test contract.
    pub initial_balance: U256,

    /// The address which will be executing all tests.
    pub sender: Address,

    /// Enables the FFI cheatcode.
    pub ffi: bool,

    /// The memory limit per EVM execution in bytes.
    /// If this limit is exceeded, a `MemoryLimitOOG` result is thrown.
    pub memory_limit: u64,

    /// Whether to enable isolation of calls.
    pub isolate: bool,

    /// Whether to disable block gas limit checks.
    pub disable_block_gas_limit: bool,
}

impl<HardforkT> Default for EvmOpts<HardforkT>
where
    HardforkT: Default,
{
    fn default() -> Self {
        Self {
            env: Env::default(),
            spec: HardforkT::default(),
            fork_url: None,
            fork_block_number: None,
            fork_retries: None,
            fork_retry_backoff: None,
            fork_headers: None,
            compute_units_per_second: None,
            no_rpc_rate_limit: false,
            initial_balance: U256::default(),
            sender: Address::default(),
            ffi: false,
            memory_limit: 0,
            isolate: false,
            disable_block_gas_limit: false,
        }
    }
}

impl<HardforkT> EvmOpts<HardforkT>
where
    HardforkT: Default,
{
    /// Configures a new `revm::Env`
    ///
    /// If a `fork_url` is set, it gets configured with settings fetched from the endpoint (chain
    /// id, )
    pub async fn evm_env<BlockT, TxT>(&self) -> eyre::Result<crate::Env<BlockT, TxT, HardforkT>>
    where
        BlockT: From<BlockEnvOpts> + Block + BlockEnvMut,
        TxT: From<TxEnvOpts>,
    {
        if let Some(ref fork_url) = self.fork_url {
            Ok(self.fork_evm_env(fork_url).await?.0)
        } else {
            Ok(self.local_evm_env())
        }
    }

    /// Returns the `revm::Env` that is configured with settings retrieved from the endpoint.
    /// And the block that was used to configure the environment.
    pub async fn fork_evm_env<BlockT, TxT>(
        &self,
        fork_url: impl AsRef<str>,
    ) -> eyre::Result<(crate::Env<BlockT, TxT, HardforkT>, AnyRpcBlock)>
    where
        BlockT: From<BlockEnvOpts> + Block + BlockEnvMut,
        TxT: From<TxEnvOpts>,
    {
        let fork_url = fork_url.as_ref();
        let provider = ProviderBuilder::new(fork_url)
            .compute_units_per_second(self.get_compute_units_per_second())
            .build()?;
        environment(
            &provider,
            self.memory_limit,
            self.env.gas_price.map(u128::from),
            self.env.chain_id,
            self.fork_block_number,
            self.sender,
            self.disable_block_gas_limit,
        )
        .await
        .wrap_err_with(|| {
            let mut msg = "could not instantiate forked environment".to_string();
            if let Ok(url) = Url::parse(fork_url)
                && let Some(provider) = url.host()
            {
                write!(msg, " with provider {provider}").unwrap();
            }
            msg
        })
    }

    /// Returns the `revm::Env` configured with only local settings
    pub fn local_evm_env<BlockT, TxT>(&self) -> crate::Env<BlockT, TxT, HardforkT>
    where
        BlockT: From<BlockEnvOpts>,
        TxT: From<TxEnvOpts>,
    {
        let cfg = configure_env(
            self.env.chain_id.unwrap_or(edr_defaults::DEV_CHAIN_ID),
            self.memory_limit,
            self.disable_block_gas_limit,
        );

        crate::Env {
            cfg,
            block: BlockEnvOpts {
                number: self.env.block_number,
                beneficiary: self.env.block_coinbase,
                timestamp: self.env.block_timestamp,
                difficulty: U256::from(self.env.block_difficulty),
                prevrandao: Some(self.env.block_prevrandao),
                basefee: self.env.block_base_fee_per_gas,
                gas_limit: self.gas_limit(),
            }
            .into(),
            tx: TxEnvOpts {
                gas_price: self.env.gas_price.unwrap_or_default().into(),
                gas_limit: self.gas_limit(),
                caller: self.sender,
                chain_id: self.env.chain_id,
            }
            .into(),
        }
    }

    /// Returns the gas limit to use
    pub fn gas_limit(&self) -> u64 {
        self.env.block_gas_limit.unwrap_or(self.env.gas_limit)
    }

    /// Returns the configured chain id, which will be
    ///   - the value of `chain_id` if set
    ///   - mainnet if `fork_url` contains "mainnet"
    ///   - the chain if `fork_url` is set and the endpoints returned its chain id successfully
    ///   - mainnet otherwise
    pub async fn get_chain_id(&self) -> u64 {
        if let Some(id) = self.env.chain_id {
            return id;
        }
        self.get_remote_chain_id()
            .await
            .unwrap_or(Chain::mainnet())
            .id()
    }

    /// Returns the available compute units per second, which will be
    /// - `u64::MAX`, if `no_rpc_rate_limit` if set (as rate limiting is disabled)
    /// - the assigned compute units, if `compute_units_per_second` is set
    /// - `ALCHEMY_FREE_TIER_CUPS` (330) otherwise
    pub fn get_compute_units_per_second(&self) -> u64 {
        if self.no_rpc_rate_limit {
            u64::MAX
        } else if let Some(cups) = self.compute_units_per_second {
            cups
        } else {
            ALCHEMY_FREE_TIER_CUPS
        }
    }

    /// Returns the chain ID from the RPC, if any.
    pub async fn get_remote_chain_id(&self) -> Option<Chain> {
        if let Some(ref url) = self.fork_url {
            trace!(?url, "retrieving chain via eth_chainId");
            let provider = ProviderBuilder::new(url.as_str())
                .compute_units_per_second(self.get_compute_units_per_second())
                .build()
                .ok()
                .unwrap_or_else(|| panic!("Failed to establish provider to {url}"));

            if let Ok(id) = provider.get_chain_id().await {
                return Some(Chain::from(id));
            }

            // Provider URLs could be of the format `{CHAIN_IDENTIFIER}-mainnet`
            // (e.g. Alchemy `opt-mainnet`, `arb-mainnet`), fallback to this method only
            // if we're not able to retrieve chain id from `RetryProvider`.
            if url.contains("mainnet") {
                trace!(?url, "auto detected mainnet chain");
                return Some(Chain::mainnet());
            }
        }

        None
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Env {
    /// The block gas limit.
    #[serde(deserialize_with = "string_or_number")]
    pub gas_limit: u64,

    /// The `CHAINID` opcode value.
    pub chain_id: Option<u64>,

    /// the tx.gasprice value during EVM execution
    ///
    /// This is an Option, so we can determine in fork mode whether to use the config's gas price
    /// (if set by user) or the remote client's gas price.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<u64>,

    /// the base fee in a block
    pub block_base_fee_per_gas: u64,

    /// the tx.origin value during EVM execution
    pub tx_origin: Address,

    /// the block.coinbase value during EVM execution
    pub block_coinbase: Address,

    /// the block.timestamp value during EVM execution
    #[serde(
        deserialize_with = "deserialize_u64_to_u256",
        serialize_with = "serialize_u64_or_u256"
    )]
    pub block_timestamp: U256,

    /// the block.number value during EVM execution"
    #[serde(
        deserialize_with = "deserialize_u64_to_u256",
        serialize_with = "serialize_u64_or_u256"
    )]
    pub block_number: U256,

    /// the block.difficulty value during EVM execution
    pub block_difficulty: u64,

    /// Previous block beacon chain random value. Before merge this field is used for `mix_hash`
    pub block_prevrandao: B256,

    /// the block.gaslimit value during EVM execution
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "string_or_number_opt"
    )]
    pub block_gas_limit: Option<u64>,

    /// EIP-170: Contract code size limit in bytes. Useful to increase this because of tests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_size_limit: Option<usize>,
}

pub struct BlockEnvOpts {
    pub number: U256,
    pub beneficiary: Address,
    pub timestamp: U256,
    pub difficulty: U256,
    pub prevrandao: Option<B256>,
    pub basefee: u64,
    pub gas_limit: u64,
}

impl From<BlockEnvOpts> for BlockEnv {
    fn from(value: BlockEnvOpts) -> Self {
        let BlockEnvOpts {
            number,
            beneficiary,
            timestamp,
            difficulty,
            prevrandao,
            basefee,
            gas_limit,
        } = value;

        Self {
            number: U256::from(number),
            beneficiary,
            timestamp: U256::from(timestamp),
            difficulty,
            prevrandao,
            basefee,
            gas_limit,
            ..Self::default()
        }
    }
}

pub struct TxEnvOpts {
    pub gas_price: u128,
    pub gas_limit: u64,
    pub chain_id: Option<u64>,
    pub caller: Address,
}

impl From<TxEnvOpts> for TxEnv {
    fn from(value: TxEnvOpts) -> Self {
        let TxEnvOpts {
            gas_price,
            gas_limit,
            chain_id,
            caller,
        } = value;

        Self {
            gas_price,
            gas_limit,
            chain_id,
            caller,
            ..Self::default()
        }
    }
}

impl From<TxEnvOpts> for OpTransaction<TxEnv> {
    fn from(value: TxEnvOpts) -> Self {
        let base = TxEnv::from(value);

        OpTransaction {
            base,
            // For Solidity tests we don't know enough information to construct an enveloped
            // transaction. Instead, we use a default value that is compatible with the
            // OpTransaction structure.
            // This means that gas estimation and balance checks won't be accurate.
            enveloped_tx: Some(vec![0x00].into()),
            deposit: DepositTransactionParts::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Gas {
    Number(u64),
    Text(String),
}

fn string_or_number<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    match Gas::deserialize(deserializer)? {
        Gas::Number(num) => Ok(num),
        Gas::Text(s) => s.parse().map_err(D::Error::custom),
    }
}

fn string_or_number_opt<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    match Option::<Gas>::deserialize(deserializer)? {
        Some(gas) => match gas {
            Gas::Number(num) => Ok(Some(num)),
            Gas::Text(s) => s.parse().map(Some).map_err(D::Error::custom),
        },
        _ => Ok(None),
    }
}

/// Deserialize into `U256` from either a `u64` or a `U256` hex string.
pub fn deserialize_u64_to_u256<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumericValue {
        U256(U256),
        U64(u64),
    }

    match NumericValue::deserialize(deserializer)? {
        NumericValue::U64(n) => Ok(U256::from(n)),
        NumericValue::U256(n) => Ok(n),
    }
}

/// Serialize `U256` as `u64` if it fits, otherwise as a hex string.
/// If the number fits into a i64, serialize it as number without quotation marks.
/// If the number fits into a u64, serialize it as a stringified number with quotation marks.
/// Otherwise, serialize it as a hex string with quotation marks.
pub fn serialize_u64_or_u256<S>(n: &U256, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // The TOML specification handles integers as i64 so the number representation is limited to
    // i64. If the number is larger than `i64::MAX` and up to `u64::MAX`, we serialize it as a
    // string to avoid losing precision.
    if let Ok(n_i64) = i64::try_from(*n) {
        serializer.serialize_i64(n_i64)
    } else if let Ok(n_u64) = u64::try_from(*n) {
        serializer.serialize_str(&n_u64.to_string())
    } else {
        serializer.serialize_str(&format!("{n:#x}"))
    }
}
