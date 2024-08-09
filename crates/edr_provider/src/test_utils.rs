use std::{num::NonZeroU64, time::SystemTime};

use edr_eth::{
    block::BlobGas,
    signature::secret_key_from_str,
    transaction::{EthTransactionRequest, IsEip4844, TransactionType},
    trie::KECCAK_NULL_RLP,
    Address, Bytes, HashMap, B256, U160, U256,
};
use edr_evm::Block;

use super::*;
use crate::{config::MiningConfig, requests::hardhat::rpc_types::ForkConfig};

pub const TEST_SECRET_KEY: &str =
    "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

// Address 0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826
pub const TEST_SECRET_KEY_SIGN_TYPED_DATA_V4: &str =
    "0xc85ef7d79691fe79573b1a7064c19c1a9819ebdbd1faaab1a8ec92344438aaf4";

pub const FORK_BLOCK_NUMBER: u64 = 18_725_000;

/// Constructs a test config with a single account with 1 ether
pub fn create_test_config<ChainSpecT: ChainSpec>() -> ProviderConfig<ChainSpecT> {
    create_test_config_with_fork(None)
}

pub fn one_ether() -> U256 {
    U256::from(10).pow(U256::from(18))
}

pub fn create_test_config_with_fork<ChainSpecT: ChainSpec>(
    fork: Option<ForkConfig>,
) -> ProviderConfig<ChainSpecT> {
    ProviderConfig {
        accounts: vec![
            AccountConfig {
                secret_key: secret_key_from_str(TEST_SECRET_KEY)
                    .expect("should construct secret key from string"),
                balance: one_ether(),
            },
            AccountConfig {
                secret_key: secret_key_from_str(TEST_SECRET_KEY_SIGN_TYPED_DATA_V4)
                    .expect("should construct secret key from string"),
                balance: one_ether(),
            },
        ],
        allow_blocks_with_same_timestamp: false,
        allow_unlimited_contract_size: false,
        bail_on_call_failure: false,
        bail_on_transaction_failure: false,
        // SAFETY: literal is non-zero
        block_gas_limit: unsafe { NonZeroU64::new_unchecked(30_000_000) },
        chain_id: 123,
        chains: HashMap::new(),
        coinbase: Address::from(U160::from(1)),
        enable_rip_7212: false,
        fork,
        genesis_accounts: HashMap::new(),
        hardfork: ChainSpecT::Hardfork::default(),
        initial_base_fee_per_gas: Some(U256::from(1000000000)),
        initial_blob_gas: Some(BlobGas {
            gas_used: 0,
            excess_gas: 0,
        }),
        initial_date: Some(SystemTime::now()),
        initial_parent_beacon_block_root: Some(KECCAK_NULL_RLP),
        min_gas_price: U256::ZERO,
        mining: MiningConfig::default(),
        network_id: 123,
        cache_dir: edr_defaults::CACHE_DIR.into(),
    }
}

/// Retrieves the pending base fee per gas from the provider data.
pub fn pending_base_fee<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        Block: Default,
        Transaction: Default
                         + TransactionValidation<
            ValidationError: From<InvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
) -> Result<U256, ProviderError<ChainSpecT>> {
    let block = data.mine_pending_block()?.block;

    let base_fee = block
        .header()
        .base_fee_per_gas
        .unwrap_or_else(|| U256::from(1));

    Ok(base_fee)
}

/// Deploys a contract with the provided code. Returns the address of the
/// contract.
pub fn deploy_contract<ChainSpecT, TimerT>(
    provider: &Provider<ChainSpecT, TimerT>,
    caller: Address,
    code: Bytes,
) -> anyhow::Result<Address>
where
    TimerT: Clone + TimeSinceEpoch,
    ChainSpecT: Debug
        + SyncProviderSpec<
            TimerT,
            Block: Clone + Default,
            HaltReason: Into<TransactionFailureReason<ChainSpecT>>,
            Transaction: Default
                             + TransactionMut
                             + TransactionType<Type: IsEip4844>
                             + TransactionValidation<
                ValidationError: From<InvalidTransaction> + PartialEq,
            >,
        >,
{
    let deploy_transaction = EthTransactionRequest {
        from: caller,
        data: Some(code),
        ..EthTransactionRequest::default()
    };

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::SendTransaction(deploy_transaction),
    ))?;

    let transaction_hash: B256 = serde_json::from_value(result.result)?;

    let result = provider.handle_request(ProviderRequest::Single(
        MethodInvocation::GetTransactionReceipt(transaction_hash),
    ))?;

    let receipt: edr_rpc_eth::receipt::Block = serde_json::from_value(result.result)?;
    let contract_address = receipt.contract_address.expect("Call must create contract");

    Ok(contract_address)
}
