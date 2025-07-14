use std::sync::Arc;

use derive_where::derive_where;
use edr_eth::{
    account::{Account, AccountInfo},
    trie::KECCAK_NULL_RLP,
    Address, Bytecode, HashMap, HashSet, B256, U256,
};
use edr_rpc_eth::{client::EthRpcClient, spec::RpcSpec};
use parking_lot::{Mutex, RwLock, RwLockUpgradableReadGuard};
use tokio::runtime;

use super::{
    remote::CachedRemoteState, RemoteState, State, StateCommit, StateDebug, StateError,
    StateMut as _, TrieState,
};
use crate::random::RandomHashGenerator;

/// A database integrating the state from a remote node and the state from a
/// local layered database.
#[derive_where(Debug)]
pub struct ForkState<ChainSpecT: RpcSpec> {
    local_state: TrieState,
    remote_state: Arc<Mutex<CachedRemoteState<ChainSpecT>>>,
    removed_storage_slots: HashSet<(Address, U256)>,
    /// A pair of the latest state root and local state root
    current_state: RwLock<(B256, B256)>,
    hash_generator: Arc<Mutex<RandomHashGenerator>>,
    removed_remote_accounts: HashSet<Address>,
}

impl<ChainSpecT: RpcSpec> ForkState<ChainSpecT> {
    /// Constructs a new instance
    pub fn new(
        runtime: runtime::Handle,
        rpc_client: Arc<EthRpcClient<ChainSpecT>>,
        hash_generator: Arc<Mutex<RandomHashGenerator>>,
        fork_block_number: u64,
        state_root: B256,
    ) -> Self {
        let remote_state = RemoteState::new(runtime, rpc_client, fork_block_number);
        let local_state = TrieState::default();

        let mut state_root_to_state = HashMap::new();
        let local_root = local_state.state_root().unwrap();
        state_root_to_state.insert(state_root, local_root);

        Self {
            local_state,
            remote_state: Arc::new(Mutex::new(CachedRemoteState::new(remote_state))),
            removed_storage_slots: HashSet::new(),
            current_state: RwLock::new((state_root, local_root)),
            hash_generator,
            removed_remote_accounts: HashSet::new(),
        }
    }

    /// Overrides the state root of the fork state.
    pub fn set_state_root(&mut self, state_root: B256) {
        let local_root = self.local_state.state_root().unwrap();

        *self.current_state.get_mut() = (state_root, local_root);
    }
}

impl<ChainSpecT: RpcSpec> Clone for ForkState<ChainSpecT> {
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

impl<ChainSpecT: RpcSpec> State for ForkState<ChainSpecT> {
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

impl<ChainSpecT: RpcSpec> StateCommit for ForkState<ChainSpecT> {
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

impl<ChainSpecT: RpcSpec> StateDebug for ForkState<ChainSpecT> {
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
        modifier: crate::state::AccountModifierFn,
    ) -> Result<AccountInfo, Self::Error> {
        self.local_state.modify_account_impl(
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
            .set_account_storage_slot_impl(address, index, value, &|| {
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
