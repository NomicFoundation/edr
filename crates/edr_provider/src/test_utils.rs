use core::fmt::Debug;
use std::{num::NonZeroU64, sync::Arc, time::SystemTime};

use anyhow::anyhow;
use edr_block_api::Block as _;
use edr_block_header::{BlobGas, BlockHeader, HeaderOverrides};
use edr_chain_l1::{
    rpc::{receipt::L1RpcTransactionReceipt, TransactionRequest},
    L1ChainSpec,
};
use edr_chain_spec::TransactionValidation;
use edr_primitives::{Address, Bytes, HashMap, B256, KECCAK_NULL_RLP, U160, U256};
use edr_signer::{public_key_to_address, secret_key_from_str, SignatureWithYParity};
use edr_solidity::contract_decoder::ContractDecoder;
use edr_transaction::{request::TransactionRequestAndSender, TxKind};
use k256::SecretKey;
use parking_lot::RwLock;
use tokio::runtime;

use crate::{
    config,
    error::ProviderErrorForChainSpec,
    observability::ObservabilityConfig,
    time::{CurrentTime, TimeSinceEpoch},
    AccountOverride, ForkConfig, MethodInvocation, NoopLogger, Provider, ProviderConfig,
    ProviderData, ProviderRequest, ProviderSpec, SyncProviderSpec,
};

pub const TEST_SECRET_KEY: &str =
    "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

// Address 0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826
pub const TEST_SECRET_KEY_SIGN_TYPED_DATA_V4: &str =
    "0xc85ef7d79691fe79573b1a7064c19c1a9819ebdbd1faaab1a8ec92344438aaf4";

pub const FORK_BLOCK_NUMBER: u64 = 18_725_000;

/// Constructs a test config in local mode with configured accounts
pub fn create_test_config<HardforkT: Default>() -> ProviderConfig<HardforkT> {
    create_test_config_with(MinimalProviderConfig::local_with_accounts())
}

/// Default base header overrides for replaying L1 blocks.
pub fn l1_base_header_overrides(
    replay_header: &BlockHeader,
) -> HeaderOverrides<edr_chain_spec::EvmSpecId> {
    HeaderOverrides {
        // Extra_data field in L1 has arbitrary additional data
        extra_data: Some(replay_header.extra_data.clone()),
        ..header_overrides(replay_header)
    }
}

/// Default header overrides for replaying L1 blocks before The Merge
pub fn l1_header_overrides_before_merge(
    replay_header: &BlockHeader,
) -> HeaderOverrides<edr_chain_spec::EvmSpecId> {
    HeaderOverrides {
        nonce: Some(replay_header.nonce),
        ..l1_base_header_overrides(replay_header)
    }
}

/// Default header overrides for replaying L1 blocks after Prague hardfork.
pub fn prague_header_overrides(
    replay_header: &BlockHeader,
) -> HeaderOverrides<edr_chain_spec::EvmSpecId> {
    HeaderOverrides {
        // EDR does not compute the `requests_hash`, as full support for EIP-7685 introduced in
        // Prague is not implemented.
        requests_hash: replay_header.requests_hash,
        ..l1_base_header_overrides(replay_header)
    }
}

/// Default header overrides for replaying blocks.
pub fn header_overrides<HardforkT: Default>(
    replay_header: &BlockHeader,
) -> HeaderOverrides<HardforkT> {
    HeaderOverrides {
        beneficiary: Some(replay_header.beneficiary),
        gas_limit: Some(replay_header.gas_limit),
        mix_hash: Some(replay_header.mix_hash),
        parent_beacon_block_root: replay_header.parent_beacon_block_root,
        state_root: Some(replay_header.state_root),
        timestamp: Some(replay_header.timestamp),
        ..HeaderOverrides::<HardforkT>::default()
    }
}

pub fn one_ether() -> U256 {
    U256::from(10).pow(U256::from(18))
}

/// Sets the [`ProviderConfig`]'s owned accounts and genesis state - computed by
/// funding each account with the provided `balance`.
pub fn set_genesis_state_with_owned_accounts<HardforkT>(
    config: &mut ProviderConfig<HardforkT>,
    owned_accounts: Vec<SecretKey>,
    balance: U256,
) {
    config.genesis_state = genesis_state_with_funded_owned_accounts(&owned_accounts, balance);
    config.owned_accounts = owned_accounts;
}

pub struct MinimalProviderConfig<HardforkT> {
    fork: Option<ForkConfig<HardforkT>>,
    genesis_state: HashMap<Address, AccountOverride>,
    observability: Option<ObservabilityConfig>,
    owned_accounts: Vec<SecretKey>,
}

impl<HardforkT> MinimalProviderConfig<HardforkT> {
    /// Fork minimal configuration without custom `owned_accounts` or
    /// `genesis_state`
    pub fn fork_empty(fork_config: ForkConfig<HardforkT>) -> MinimalProviderConfig<HardforkT> {
        MinimalProviderConfig {
            fork: Some(fork_config),
            genesis_state: HashMap::default(),
            observability: None,
            owned_accounts: vec![],
        }
    }

    /// Local minimal configuration without custom `owned_accounts` or
    /// `genesis_state`
    pub fn local_empty() -> MinimalProviderConfig<HardforkT> {
        MinimalProviderConfig {
            fork: None,
            genesis_state: HashMap::default(),
            observability: None,
            owned_accounts: vec![],
        }
    }

    /// Fork minimal configuration with default custom `owned_accounts` or
    /// `genesis_state`
    pub fn fork_with_accounts(
        fork_config: ForkConfig<HardforkT>,
    ) -> MinimalProviderConfig<HardforkT> {
        let owned_accounts = Self::default_accounts();
        MinimalProviderConfig {
            fork: Some(fork_config),
            genesis_state: genesis_state_with_funded_owned_accounts(&owned_accounts, one_ether()),
            observability: None,
            owned_accounts,
        }
    }
    /// Local minimal configuration with default custom `owned_accounts` or
    /// `genesis_state`
    pub fn local_with_accounts() -> MinimalProviderConfig<HardforkT> {
        let owned_accounts = Self::default_accounts();
        MinimalProviderConfig {
            fork: None,
            genesis_state: genesis_state_with_funded_owned_accounts(&owned_accounts, one_ether()),
            observability: None,
            owned_accounts,
        }
    }

    /// Adds the provided `observability_config` to the instance.
    pub fn with_observability(&mut self, observability_config: ObservabilityConfig) {
        self.observability = Some(observability_config);
    }

    fn default_accounts() -> Vec<SecretKey> {
        // This is test code, it's ok to use `DangerousSecretKeyStr`
        #[allow(deprecated)]
        use edr_signer::DangerousSecretKeyStr;

        // This is test code, it's ok to use `DangerousSecretKeyStr`
        // Can't use `edr_test_utils` as a dependency here.
        vec![
            #[allow(deprecated)]
            secret_key_from_str(DangerousSecretKeyStr(TEST_SECRET_KEY))
                .expect("should construct secret key from string"),
            #[allow(deprecated)]
            secret_key_from_str(DangerousSecretKeyStr(TEST_SECRET_KEY_SIGN_TYPED_DATA_V4))
                .expect("should construct secret key from string"),
        ]
    }
}

pub fn create_test_config_with<HardforkT: Default>(
    config: MinimalProviderConfig<HardforkT>,
) -> ProviderConfig<HardforkT> {
    ProviderConfig {
        allow_blocks_with_same_timestamp: false,
        allow_unlimited_contract_size: false,
        bail_on_call_failure: false,
        bail_on_transaction_failure: false,
        base_fee_params: None,
        // SAFETY: literal is non-zero
        block_gas_limit: unsafe { NonZeroU64::new_unchecked(30_000_000) },
        chain_id: 123,
        coinbase: Address::from(U160::from(1)),
        fork: config.fork,
        genesis_state: config.genesis_state,
        hardfork: HardforkT::default(),
        initial_base_fee_per_gas: Some(1000000000),
        initial_blob_gas: Some(BlobGas {
            gas_used: 0,
            excess_gas: 0,
        }),
        initial_date: Some(SystemTime::now()),
        initial_parent_beacon_block_root: Some(KECCAK_NULL_RLP),
        min_gas_price: 0,
        mining: config::Mining::default(),
        network_id: 123,
        observability: config.observability.unwrap_or_default(),
        owned_accounts: config.owned_accounts,
        precompile_overrides: HashMap::default(),
        transaction_gas_cap: None,
    }
}
/// Retrieves the pending base fee per gas from the provider data.
pub fn pending_base_fee<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        SignedTransaction: Default + TransactionValidation<ValidationError: PartialEq>,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
) -> Result<u128, ProviderErrorForChainSpec<ChainSpecT>> {
    let block = data.mine_pending_block()?.block_and_state.block;

    let base_fee = block.block_header().base_fee_per_gas.unwrap_or(1);

    Ok(base_fee)
}

/// Deploys a contract with the provided code. Returns the address of the
/// contract.
pub fn deploy_contract<TimerT>(
    provider: &Provider<L1ChainSpec, TimerT>,
    caller: Address,
    code: Bytes,
) -> anyhow::Result<Address>
where
    TimerT: Clone + TimeSinceEpoch,
{
    let deploy_transaction = TransactionRequest {
        from: caller,
        data: Some(code),
        ..TransactionRequest::default()
    };

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(deploy_transaction),
    ))?;

    let transaction_hash: B256 = serde_json::from_value(result.result)?;

    let result = provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::GetTransactionReceipt(transaction_hash),
    ))?;

    let receipt: L1RpcTransactionReceipt = serde_json::from_value(result.result)?;
    let contract_address = receipt.contract_address.expect("Call must create contract");

    Ok(contract_address)
}

/// Fixture for testing `ProviderData`.
pub struct ProviderTestFixture<ChainSpecT: ProviderSpec<CurrentTime>> {
    _runtime: runtime::Runtime,
    pub config: ProviderConfig<ChainSpecT::Hardfork>,
    pub provider_data: ProviderData<ChainSpecT, CurrentTime>,
    pub impersonated_account: Address,
}

impl<ChainSpecT> ProviderTestFixture<ChainSpecT>
where
    ChainSpecT: Debug + SyncProviderSpec<CurrentTime, Hardfork: Default>,
{
    /// Creates a new `ProviderTestFixture` with a local provider.
    pub fn new_local() -> anyhow::Result<Self> {
        Self::with_config(MinimalProviderConfig::local_with_accounts())
    }

    /// Creates a new `ProviderTestFixture` with a forked provider.
    pub fn new_forked(url: Option<String>) -> anyhow::Result<Self> {
        use edr_test_utils::env::json_rpc_url_provider;

        Self::with_config(MinimalProviderConfig::fork_with_accounts(ForkConfig {
            block_number: None,
            cache_dir: edr_defaults::CACHE_DIR.into(),
            chain_overrides: HashMap::default(),
            http_headers: None,
            url: url.unwrap_or(json_rpc_url_provider::ethereum_mainnet()),
        }))
    }

    fn with_config(config: MinimalProviderConfig<ChainSpecT::Hardfork>) -> anyhow::Result<Self> {
        let config = create_test_config_with(config);

        let runtime = runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .thread_name("provider-data-test")
            .build()?;

        Self::new(runtime, config)
    }

    pub fn new(
        runtime: tokio::runtime::Runtime,
        mut config: ProviderConfig<ChainSpecT::Hardfork>,
    ) -> anyhow::Result<Self> {
        let logger = Box::<NoopLogger<ChainSpecT, CurrentTime>>::default();
        let subscription_callback_noop = Box::new(|_| ());

        let impersonated_account = Address::random();
        config.genesis_state.insert(
            impersonated_account,
            AccountOverride {
                balance: Some(one_ether()),
                ..AccountOverride::default()
            },
        );

        let mut provider_data = ProviderData::<ChainSpecT>::new(
            runtime.handle().clone(),
            logger,
            subscription_callback_noop,
            config.clone(),
            Arc::new(RwLock::<ContractDecoder>::default()),
            CurrentTime,
        )?;

        provider_data.impersonate_account(impersonated_account);

        Ok(Self {
            _runtime: runtime,
            config,
            provider_data,
            impersonated_account,
        })
    }

    /// Retrieves the nth local account.
    ///
    /// # Panics
    ///
    /// Panics if there are not enough local accounts
    pub fn nth_local_account(&self, index: usize) -> anyhow::Result<Address> {
        self.provider_data
            .accounts()
            .nth(index)
            .copied()
            .ok_or(anyhow!("the requested local account does not exist"))
    }
}

impl ProviderTestFixture<L1ChainSpec> {
    pub fn dummy_transaction_request(
        &self,
        local_account_index: usize,
        gas_limit: u64,
        nonce: Option<u64>,
    ) -> anyhow::Result<TransactionRequestAndSender<edr_chain_l1::L1TransactionRequest>> {
        let request = edr_chain_l1::L1TransactionRequest::Eip155(edr_chain_l1::request::Eip155 {
            kind: TxKind::Call(Address::ZERO),
            gas_limit,
            gas_price: 42_000_000_000_u128,
            value: U256::from(1),
            input: Bytes::default(),
            nonce: nonce.unwrap_or(0),
            chain_id: self.config.chain_id,
        });

        let sender = self.nth_local_account(local_account_index)?;
        Ok(TransactionRequestAndSender { request, sender })
    }

    pub fn impersonated_dummy_transaction(
        &self,
    ) -> anyhow::Result<edr_chain_l1::L1SignedTransaction> {
        let mut transaction = self.dummy_transaction_request(0, 30_000, None)?;
        transaction.sender = self.impersonated_account;

        Ok(self.provider_data.sign_transaction_request(transaction)?)
    }

    pub fn signed_dummy_transaction(
        &self,
        local_account_index: usize,
        nonce: Option<u64>,
    ) -> anyhow::Result<edr_chain_l1::L1SignedTransaction> {
        let transaction = self.dummy_transaction_request(local_account_index, 30_000, nonce)?;
        Ok(self.provider_data.sign_transaction_request(transaction)?)
    }
}

/// Signs an authorization with the provided secret key.
pub fn sign_authorization(
    authorization: edr_eip7702::Authorization,
    secret_key: &SecretKey,
) -> anyhow::Result<edr_eip7702::SignedAuthorization> {
    let signature = SignatureWithYParity::with_message(authorization.signature_hash(), secret_key)?;

    Ok(authorization.into_signed(signature.into_inner()))
}

/// Constructs a genesis state by funding the owned accounts with the provided
/// `balance`.
fn genesis_state_with_funded_owned_accounts(
    owned_accounts: &[SecretKey],
    balance: U256,
) -> HashMap<Address, AccountOverride> {
    owned_accounts
        .iter()
        .map(|secret_key| {
            let address = public_key_to_address(secret_key.public_key());
            let account_override = AccountOverride {
                balance: Some(balance),
                ..AccountOverride::default()
            };

            (address, account_override)
        })
        .collect()
}
