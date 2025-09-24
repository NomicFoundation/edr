// Part of this code was adapted from foundry and is distributed under their
// licenses:
// - https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/LICENSE-APACHE
// - https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/LICENSE-MIT
// For the original context see: https://github.com/foundry-rs/foundry/blob/01b16238ff87dc7ca8ee3f5f13e389888c2a2ee4/anvil/core/src/eth/trie.rs

#![warn(missing_docs)]
//! Ethereum trie functions

use edr_primitives::B256;
use hash256_std_hasher::Hash256StdHasher;
use sha3::{
    digest::generic_array::{typenum::consts::U32, GenericArray},
    Digest, Keccak256,
};

/// Generates a trie root hash for a vector of key-value tuples
pub fn trie_root<I, K, V>(input: I) -> B256
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<[u8]> + Ord,
    V: AsRef<[u8]>,
{
    B256::from_slice(triehash::trie_root::<KeccakHasher, _, _, _>(input).as_ref())
}

/// Generates a key-hashed (secure) trie root hash for a vector of key-value
/// tuples.
pub fn sec_trie_root<I, K, V>(input: I) -> B256
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<[u8]>,
    V: AsRef<[u8]>,
{
    B256::from_slice(triehash::sec_trie_root::<KeccakHasher, _, _, _>(input).as_ref())
}

/// Generates a trie root hash for a vector of values
pub fn ordered_trie_root<I, V>(input: I) -> B256
where
    I: IntoIterator<Item = V>,
    V: AsRef<[u8]>,
{
    B256::from_slice(triehash::ordered_trie_root::<KeccakHasher, I>(input).as_ref())
}

struct KeccakHasher;

impl hash_db::Hasher for KeccakHasher {
    type Out = GenericArray<u8, U32>;

    type StdHasher = Hash256StdHasher;

    const LENGTH: usize = 32;

    fn hash(x: &[u8]) -> Self::Out {
        Keccak256::digest(x)
    }
}
