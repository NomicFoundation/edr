use std::sync::Arc;

use alloy_rlp::Decodable;
use edr_primitives::{Address, B256};
use edr_state_api::account::{AccountInfo, BasicAccount};

use crate::{persistent_db::PersistentMemoryDB, query::TrieQuery};

#[derive(Debug)]
pub(super) struct PersistentAccountTrie {
    db: Arc<PersistentMemoryDB>,
    root: B256,
}

impl PersistentAccountTrie {
    /// Retrieves the account for the given address, if it exists.
    pub fn account(&self, address: &Address) -> Option<BasicAccount> {
        self.trie_query().get(address).map(|encoded_account| {
            BasicAccount::decode(&mut encoded_account.as_slice()).expect("Valid RLP")
        })
    }

    /// Create a helper struct that allows setting and removing multiple
    /// accounts and then updates the state root when dropped.
    pub fn mutate(&mut self) -> AccountTrieMutation<'_> {
        let trie_query = self.trie_query();
        AccountTrieMutation {
            account_trie: self,
            trie_query,
        }
    }

    pub fn root(&self) -> B256 {
        self.root
    }

    pub fn trie_query(&self) -> TrieQuery {
        TrieQuery::new(Arc::clone(&self.db), &self.root)
    }
}

impl Clone for PersistentAccountTrie {
    fn clone(&self) -> Self {
        Self {
            db: Arc::new((*self.db).clone()),
            root: self.root,
        }
    }
}

impl Default for PersistentAccountTrie {
    fn default() -> Self {
        let db = Arc::new(PersistentMemoryDB::default());
        let mut trie = TrieQuery::empty(Arc::clone(&db));
        let root = trie.root();

        Self { db, root }
    }
}

/// A helper struct that lets us update multiple accounts and updates the
/// account trie root when dropped.
pub struct AccountTrieMutation<'a> {
    account_trie: &'a mut PersistentAccountTrie,
    trie_query: TrieQuery,
}

impl AccountTrieMutation<'_> {
    pub fn account(&self, address: &Address) -> Option<BasicAccount> {
        self.account_trie.account(address)
    }

    pub fn remove_account(&mut self, address: &Address) {
        self.trie_query.remove(address);
    }

    pub fn insert_account_info_with_storage_root(
        &mut self,
        address: &Address,
        account_info: &AccountInfo,
        storage_root: B256,
    ) {
        let account = BasicAccount::from((account_info, storage_root));
        self.insert_basic_account(address, account);
    }

    pub fn insert_basic_account(&mut self, address: &Address, account: BasicAccount) {
        self.trie_query.insert(address, account);
    }
}

impl Drop for AccountTrieMutation<'_> {
    fn drop(&mut self) {
        self.account_trie.root = self.trie_query.root();
    }
}
