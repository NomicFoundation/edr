use std::{collections::BTreeMap, sync::Arc};

use alloy_rlp::Decodable;
use alloy_trie::Nibbles;
use edr_primitives::{keccak256, Bytes, StorageKey, B256, U256};
use hasher::{Hasher, HasherKeccak};
use revm_state::EvmStorage;

use crate::{
    persistent_db::PersistentMemoryDB,
    query::{build_proof_nodes, TrieQuery},
};

#[derive(Debug)]
pub(super) struct StorageTrie {
    db: Arc<PersistentMemoryDB>,
    root: B256,
}

impl<'a> StorageTrie {
    pub fn mutate(&'a mut self) -> StorageTrieMutation<'a> {
        let trie_query = self.trie_query();
        StorageTrieMutation {
            storage_trie: self,
            trie_query,
        }
    }

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub fn storage_slot(&self, index: &U256) -> Option<U256> {
        self.trie_query()
            .get(index.to_be_bytes::<32>())
            .map(decode_u256)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub fn storage(&self) -> BTreeMap<B256, U256> {
        self.trie_query()
            .iter()
            .map(|(hashed_index, encoded_value)| {
                (B256::from_slice(&hashed_index), decode_u256(encoded_value))
            })
            .collect()
    }

    pub fn root(&self) -> B256 {
        self.root
    }

    pub fn generate_proof(&self, keys: &[StorageKey]) -> Vec<Vec<Bytes>> {
        let keys: Vec<_> = keys
            .iter()
            .map(|key| Nibbles::unpack(keccak256(key)))
            .collect();

        let all_proof_nodes = build_proof_nodes(self.trie_query(), keys.clone());

        keys.iter()
            .map(|proof_key| {
                // Map over keys so that the result is guaranteed to be in order.
                all_proof_nodes
                    .matching_nodes_sorted(proof_key)
                    .into_iter()
                    .map(|(_, node)| node)
                    .collect()
            })
            .collect()
    }

    fn trie_query(&'a self) -> TrieQuery {
        TrieQuery::new(Arc::clone(&self.db), &self.root)
    }
}

impl Clone for StorageTrie {
    fn clone(&self) -> Self {
        Self {
            db: Arc::new((*self.db).clone()),
            root: self.root,
        }
    }
}

impl Default for StorageTrie {
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    fn default() -> Self {
        let db = Arc::new(PersistentMemoryDB::default());
        let mut trie = TrieQuery::empty(Arc::clone(&db));
        let root = trie.root();

        Self { db, root }
    }
}

pub(super) struct StorageTrieMutation<'a> {
    storage_trie: &'a mut StorageTrie,
    trie_query: TrieQuery,
}

impl StorageTrieMutation<'_> {
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self)))]
    pub fn set_storage_slots(&mut self, storage: &EvmStorage) {
        storage.iter().for_each(|(index, value)| {
            self.set_storage_slot(index, &value.present_value);
        });
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self)))]
    pub fn set_storage_slot(&mut self, index: &U256, value: &U256) -> Option<U256> {
        let hashed_index = HasherKeccak::new().digest(&index.to_be_bytes::<32>());

        let old_value = self
            .trie_query
            .get_hashed_key(&hashed_index)
            .map(decode_u256);

        if value.is_zero() {
            if old_value.is_some() {
                self.trie_query.remove_hashed_key(&hashed_index);
            }
        } else {
            self.trie_query.insert_hashed_key(hashed_index, value);
        }

        old_value
    }
}

impl Drop for StorageTrieMutation<'_> {
    fn drop(&mut self) {
        self.storage_trie.root = self.trie_query.root();
    }
}

fn decode_u256(encoded_value: Vec<u8>) -> U256 {
    U256::decode(&mut encoded_value.as_slice()).expect("Valid RLP")
}
