use std::sync::Arc;

use alloy_rpc_types::EIP1186AccountProofResponse;
use derive_where::derive_where;
use edr_chain_spec_rpc::{RpcBlockChainSpec, RpcChainSpec, RpcEthBlock};
use edr_primitives::{Address, Bytecode, HashMap, HashSet, B256, KECCAK_NULL_RLP, U256};
use edr_rpc_eth::client::EthRpcClient;
use edr_state_api::{
    account::{Account, AccountInfo},
    AccountModifierFn, State, StateCommit, StateDebug, StateError, StateMut as _, StateProof,
};
use edr_state_persistent_trie::PersistentStateTrie;
use edr_state_remote::{CachedRemoteState, RemoteState};
use edr_utils::random::RandomHashGenerator;
use parking_lot::{Mutex, RwLock, RwLockUpgradableReadGuard};
use serde::{de::DeserializeOwned, Serialize};
use tokio::runtime;

/// Helper type for a chain-specific [`ForkedState`].
pub type ForkedStateForChainSpec<ChainSpecT> = ForkedState<
    ChainSpecT,
    <ChainSpecT as RpcChainSpec>::RpcReceipt,
    <ChainSpecT as RpcChainSpec>::RpcTransaction,
>;

/// A database integrating the state from a remote node and the state from a
/// local layered database.
#[derive_where(Debug)]
pub struct ForkedState<
    RpcBlockChainSpecT: RpcBlockChainSpec,
    RpcReceiptT: DeserializeOwned + Serialize,
    RpcTransactionT: DeserializeOwned + Serialize,
> {
    local_state: PersistentStateTrie,
    remote_state: Arc<Mutex<CachedRemoteState<RpcBlockChainSpecT, RpcReceiptT, RpcTransactionT>>>,
    removed_storage_slots: HashSet<(Address, U256)>,
    /// A pair of the latest state root and local state root
    current_state: RwLock<(B256, B256)>,
    hash_generator: Arc<Mutex<RandomHashGenerator>>,
    removed_remote_accounts: HashSet<Address>,
}

impl<
        RpcBlockT: RpcBlockChainSpec,
        RpcReceiptT: DeserializeOwned + Serialize,
        RpcTransactionT: DeserializeOwned + Serialize,
    > ForkedState<RpcBlockT, RpcReceiptT, RpcTransactionT>
{
    /// Constructs a new instance
    pub fn new(
        runtime: runtime::Handle,
        rpc_client: Arc<EthRpcClient<RpcBlockT, RpcReceiptT, RpcTransactionT>>,
        hash_generator: Arc<Mutex<RandomHashGenerator>>,
        fork_block_number: u64,
        state_root: B256,
    ) -> Self {
        let remote_state = RemoteState::new(runtime, rpc_client, fork_block_number);
        let local_state = PersistentStateTrie::default();

        let mut state_root_to_state = HashMap::new();
        let local_root = local_state.state_root().unwrap();
        state_root_to_state.insert(state_root, local_root);

        Self {
            local_state,
            remote_state: Arc::new(Mutex::new(CachedRemoteState::new(remote_state))),
            removed_storage_slots: HashSet::default(),
            current_state: RwLock::new((state_root, local_root)),
            hash_generator,
            removed_remote_accounts: HashSet::default(),
        }
    }

    /// Overrides the state root of the fork state.
    pub fn set_state_root(&mut self, state_root: B256) {
        let local_root = self.local_state.state_root().unwrap();

        *self.current_state.get_mut() = (state_root, local_root);
    }
}

impl<
        RpcBlockT: RpcBlockChainSpec,
        RpcReceiptT: DeserializeOwned + Serialize,
        RpcTransactionT: DeserializeOwned + Serialize,
    > Clone for ForkedState<RpcBlockT, RpcReceiptT, RpcTransactionT>
{
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    fn clone(&self) -> Self {
        Self {
            local_state: self.local_state.clone(),
            remote_state: self.remote_state.clone(),
            removed_storage_slots: self.removed_storage_slots.clone(),
            current_state: RwLock::new(*self.current_state.read()),
            hash_generator: self.hash_generator.clone(),
            removed_remote_accounts: self.removed_remote_accounts.clone(),
        }
    }
}

impl<
        RpcBlockT: RpcBlockChainSpec<RpcBlock<B256>: RpcEthBlock>,
        RpcReceiptT: DeserializeOwned + Serialize,
        RpcTransactionT: DeserializeOwned + Serialize,
    > State for ForkedState<RpcBlockT, RpcReceiptT, RpcTransactionT>
{
    type Error = StateError;

    fn basic(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        if let Some(local) = self.local_state.basic(address)? {
            Ok(Some(local))
        } else if self.removed_remote_accounts.contains(&address) {
            Ok(None)
        } else {
            self.remote_state.lock().basic_mut(address)
        }
    }

    fn code_by_hash(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        if let Ok(layered) = self.local_state.code_by_hash(code_hash) {
            Ok(layered)
        } else {
            self.remote_state.lock().code_by_hash_mut(code_hash)
        }
    }

    fn storage(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let local = self.local_state.storage(address, index)?;
        if local != U256::ZERO || self.removed_storage_slots.contains(&(address, index)) {
            Ok(local)
        } else {
            self.remote_state.lock().storage_mut(address, index)
        }
    }
}

impl<
        RpcBlockT: RpcBlockChainSpec,
        RpcReceiptT: DeserializeOwned + Serialize,
        RpcTransactionT: DeserializeOwned + Serialize,
    > StateCommit for ForkedState<RpcBlockT, RpcReceiptT, RpcTransactionT>
{
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        changes.iter().for_each(|(address, account)| {
            account.storage.iter().for_each(|(index, value)| {
                // We never need to remove zero entries as a "removed" entry means that the
                // lookup for a value in the local state succeeded.
                if value.present_value() == U256::ZERO {
                    self.removed_storage_slots.insert((*address, *index));
                }
            });
        });

        self.local_state.commit(changes);
    }
}

impl<
        RpcBlockT: RpcBlockChainSpec<RpcBlock<B256>: RpcEthBlock>,
        RpcReceiptT: DeserializeOwned + Serialize,
        RpcTransactionT: DeserializeOwned + Serialize,
    > StateDebug for ForkedState<RpcBlockT, RpcReceiptT, RpcTransactionT>
{
    type Error = StateError;

    fn account_storage_root(&self, _address: &Address) -> Result<Option<B256>, Self::Error> {
        // HACK: Hardhat ignores the storage root, so we set it to the default value
        Ok(Some(KECCAK_NULL_RLP))
    }

    fn insert_account(
        &mut self,
        address: Address,
        account_info: AccountInfo,
    ) -> Result<(), Self::Error> {
        self.local_state.insert_account(address, account_info)
    }

    fn modify_account(
        &mut self,
        address: Address,
        modifier: AccountModifierFn,
    ) -> Result<AccountInfo, Self::Error> {
        self.local_state.modify_account_or_else(
            address,
            modifier,
            &|| {
                self.remote_state.lock().basic_mut(address)?.map_or_else(
                    || {
                        Ok(AccountInfo {
                            code: None,
                            ..AccountInfo::default()
                        })
                    },
                    Result::Ok,
                )
            },
            &|code_hash| self.remote_state.lock().code_by_hash_mut(code_hash),
        )
    }

    fn remove_account(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        if let Some(account_info) = self.local_state.remove_account(address)? {
            Ok(Some(account_info))
        } else if self.removed_remote_accounts.contains(&address) {
            Ok(None)
        } else if let Some(account_info) = self.remote_state.lock().basic_mut(address)? {
            self.removed_remote_accounts.insert(address);
            Ok(Some(account_info))
        } else {
            Ok(None)
        }
    }

    fn serialize(&self) -> String {
        self.local_state.serialize()
    }

    fn set_account_storage_slot(
        &mut self,
        address: Address,
        index: U256,
        value: U256,
    ) -> Result<U256, Self::Error> {
        // We never need to remove zero entries as a "removed" entry means that the
        // lookup for a value in the local state succeeded.
        if value == U256::ZERO {
            self.removed_storage_slots.insert((address, index));
        }

        self.local_state
            .set_account_storage_slot_or_else(address, index, value, &|| {
                self.remote_state.lock().basic_mut(address)?.map_or_else(
                    || {
                        Ok(AccountInfo {
                            code: None,
                            ..AccountInfo::default()
                        })
                    },
                    Ok,
                )
            })
    }

    fn state_root(&self) -> Result<B256, Self::Error> {
        let local_root = self.local_state.state_root().unwrap();

        let current_state = self.current_state.upgradable_read();

        Ok(if local_root == current_state.1 {
            current_state.0
        } else {
            let next_state_root = self.hash_generator.lock().next_value();

            *RwLockUpgradableReadGuard::upgrade(current_state) = (next_state_root, local_root);

            next_state_root
        })
    }
}

impl<
        RpcBlockT: RpcBlockChainSpec<RpcBlock<B256>: RpcEthBlock>,
        RpcReceiptT: DeserializeOwned + Serialize,
        RpcTransactionT: DeserializeOwned + Serialize,
    > StateProof for ForkedState<RpcBlockT, RpcReceiptT, RpcTransactionT>
{
    /// The state's error type.
    type Error = StateError;

    fn proof(
        &self,
        _address: Address,
        _storage_keys: Vec<B256>,
    ) -> Result<EIP1186AccountProofResponse, Self::Error> {
        Err(StateError::UnsupportedGetProof)
    }
}
#[cfg(all(test, feature = "test-remote"))]
mod tests {
    use std::{
        ops::{Deref, DerefMut},
        str::FromStr,
    };

    use edr_chain_l1::L1ChainSpec;
    use edr_eth::PreEip1898BlockSpec;
    use edr_rpc_eth::client::EthRpcClientForChainSpec;
    use edr_test_utils::env::json_rpc_url_provider;

    use super::*;

    const FORK_BLOCK: u64 = 16220843;

    struct TestForkState {
        fork_state: ForkedStateForChainSpec<L1ChainSpec>,
        // We need to keep it around as long as the fork state is alive
        _tempdir: tempfile::TempDir,
    }

    impl TestForkState {
        /// Constructs a fork state for testing purposes.
        ///
        /// # Panics
        ///
        /// If the function is called without a `tokio::Runtime` existing.
        async fn new() -> Self {
            let hash_generator = Arc::new(Mutex::new(RandomHashGenerator::with_seed("seed")));

            let tempdir = tempfile::tempdir().expect("can create tempdir");

            let runtime = runtime::Handle::current();
            let rpc_client = EthRpcClientForChainSpec::<L1ChainSpec>::new(
                &json_rpc_url_provider::ethereum_mainnet(),
                tempdir.path().to_path_buf(),
                None,
            )
            .expect("url ok");

            let block = rpc_client
                .get_block_by_number(PreEip1898BlockSpec::Number(FORK_BLOCK))
                .await
                .expect("failed to retrieve block by number")
                .expect("block should exist");

            let fork_state = ForkedState::new(
                runtime,
                Arc::new(rpc_client),
                hash_generator,
                FORK_BLOCK,
                block.state_root,
            );

            Self {
                fork_state,
                _tempdir: tempdir,
            }
        }
    }

    impl Deref for TestForkState {
        type Target = ForkedStateForChainSpec<L1ChainSpec>;

        fn deref(&self) -> &Self::Target {
            &self.fork_state
        }
    }

    impl DerefMut for TestForkState {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.fork_state
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn basic_success() {
        let fork_state = TestForkState::new().await;

        let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
            .expect("failed to parse address");

        let account_info = fork_state
            .basic(dai_address)
            .expect("should have succeeded");
        assert!(account_info.is_some());

        let account_info = account_info.unwrap();
        assert_eq!(account_info.balance, U256::from(0));
        assert_eq!(account_info.nonce, 1);
        assert_eq!(
            account_info.code_hash,
            B256::from_str("0x4e36f96ee1667a663dfaac57c4d185a0e369a3a217e0079d49620f34f85d1ac7")
                .expect("failed to parse")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn remove_remote_account_success() {
        let mut fork_state = TestForkState::new().await;

        let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
            .expect("failed to parse address");

        fork_state.remove_account(dai_address).unwrap();

        assert_eq!(fork_state.basic(dai_address).unwrap(), None);
    }
}
