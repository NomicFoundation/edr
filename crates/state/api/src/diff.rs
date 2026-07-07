use edr_primitives::{Address, U256};

use crate::{
    account::{Account, AccountInfo, AccountStatus},
    EvmState, EvmStorage, EvmStorageSlot,
};

/// The difference between two states, which can be applied to a state to get
/// the new state using [`crate::StateCommit::commit`].
#[derive(Clone, Debug, Default)]
pub struct StateDiff {
    inner: EvmState,
}

impl StateDiff {
    /// Applies a single change to this instance, combining it with any existing
    /// change.
    pub fn apply_account_change(&mut self, address: Address, account_info: AccountInfo) {
        self.inner
            .entry(address)
            .and_modify(|account| {
                account.info = account_info.clone();
            })
            .or_insert(Account {
                info: account_info.clone(),
                original_info: Box::new(account_info),
                storage: EvmStorage::default(),
                status: AccountStatus::Touched,
                transaction_id: 0,
            });
    }

    /// Applies a single storage change to this instance, combining it with any
    /// existing change.
    ///
    /// If the account corresponding to the specified address hasn't been
    /// modified before, either the value provided in `account_info` will be
    /// used, or alternatively a default account will be created.
    pub fn apply_storage_change(
        &mut self,
        address: Address,
        index: U256,
        slot: EvmStorageSlot,
        account_info: Option<AccountInfo>,
    ) {
        self.inner
            .entry(address)
            .and_modify(|account| {
                account.storage.insert(index, slot.clone());
            })
            .or_insert_with(|| {
                let storage: EvmStorage = std::iter::once((index, slot.clone())).collect();

                let info = account_info.unwrap_or_default();
                Account {
                    info: info.clone(),
                    original_info: Box::new(info),
                    storage,
                    status: AccountStatus::Created | AccountStatus::Touched,
                    transaction_id: 0,
                }
            });
    }

    /// Applies a state diff to this instance, combining with any and all
    /// existing changes.
    pub fn apply_diff(&mut self, diff: EvmState) {
        for (address, account_diff) in diff {
            self.inner
                .entry(address)
                .and_modify(|account| {
                    account.info = account_diff.info.clone();
                    account.status.insert(account_diff.status);
                    account.storage.extend(account_diff.storage.clone());
                })
                .or_insert(account_diff);
        }
    }

    /// Retrieves the inner hash map.
    pub fn as_inner(&self) -> &EvmState {
        &self.inner
    }
}

impl From<EvmState> for StateDiff {
    fn from(value: EvmState) -> Self {
        Self { inner: value }
    }
}

impl From<StateDiff> for EvmState {
    fn from(value: StateDiff) -> Self {
        value.inner
    }
}
