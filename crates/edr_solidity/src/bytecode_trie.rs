use std::{
    cmp::Ordering,
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
    sync::Arc,
};

use crate::build_model::ContractMetadata;

/// The key for an item in the bytecode trie.
pub trait TrieKeyTrait {
    fn key(&self) -> &[u8];
}

/// The result of searching for a bytecode in a [`BytecodeTrie`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrieSearch<'a, T> {
    /// An exact match was found.
    ExactHit(T),
    /// No exact match found; a node with the longest prefix is returned.
    LongestPrefixNode {
        node: &'a BytecodeTrie<T>,
        match_: Option<T>,
        diff_index: usize,
    },
}

/// This class represent a somewhat special compressed Trie of bytecodes.
///
/// What makes it special is that every node has a set of all of its
/// descendants.
///
/// Wrap the item type `T` in an `Arc` or `Rc` if it's expensive to clone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeTrie<T> {
    /// The purpose of this is to keep track of the order in which descendants
    /// are added.
    pub descendants: Vec<T>,
    /// If set, there is an exact match at the end of the prefix for the node.
    match_: Option<T>,
    child_nodes: HashMap<u8, Box<BytecodeTrie<T>>>,
    prefix: TriePrefix<T>,
}

impl<T: Clone + TrieKeyTrait> BytecodeTrie<T> {
    /// Create a new trie root.
    pub fn new_root() -> BytecodeTrie<T> {
        Self {
            child_nodes: HashMap::new(),
            descendants: Vec::new(),
            match_: None,
            prefix: TriePrefix::new_root(),
        }
    }
    pub fn add(&mut self, new_item: T) {
        let mut cursor = self;

        for (index, new_key_byte) in new_item.key().iter().copied().enumerate() {
            if index < cursor.prefix.range_end {
                // If there is a mismatch with the prefix of the cursor, we have to add a split
                // node
                if new_key_byte != cursor.prefix.key.key()[index] {
                    let split_node = Self::new_split_node(index, new_item.clone());
                    let node_to_split = std::mem::replace(cursor, split_node);
                    cursor.fill_split_node(node_to_split, new_item.clone());

                    // `Self::fill_split_node` adds the item as a child of the split node so we can
                    // stop here.
                    return;
                }
            } else {
                cursor.descendants.push(new_item.clone());

                match cursor.child_nodes.entry(new_key_byte) {
                    Entry::Occupied(entry) => {
                        cursor = entry.into_mut();
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(Box::new(Self::new_leaf(new_item.clone())));
                        return;
                    }
                }
            }
        }

        // If multiple contracts with the exact same bytecode are added we keep
        // the last of them. Note that this includes the metadata hash,
        // so the chances of happening are pretty remote, except in
        // super artificial cases that we have in our test suite.
        cursor.match_ = Some(new_item);
    }

    /// Searches for a bytecode. If it's an exact match, it is returned. If
    /// there's no match, but a prefix of the code is found in the trie, the
    /// node of the longest prefix is returned. If the entire code is
    /// covered by the trie, and there's no match, we return None.
    pub fn search(&self, key: &[u8], current_code_byte: usize) -> Option<TrieSearch<'_, T>> {
        if current_code_byte > key.len() {
            return None;
        }

        let mut cursor = self;
        let mut index = current_code_byte;

        while index < key.len() {
            if index < cursor.prefix.range_end {
                if key[index] != cursor.prefix.key.key()[index] {
                    // Cursor cannot be root here, because the root's prefix ends at index 0.
                    return Some(TrieSearch::LongestPrefixNode {
                        node: cursor,
                        diff_index: index,
                        // We're not yet at the end of the range, so it cannot be a match.
                        match_: None,
                    });
                }
            } else if let Some(node) = cursor.child_nodes.get(&key[index]) {
                cursor = node;
            } else if !cursor.is_root() {
                return Some(TrieSearch::LongestPrefixNode {
                    node: cursor,
                    diff_index: index,
                    match_: cursor.match_.clone(),
                });
            }

            index += 1;
        }

        cursor.match_.as_ref().and_then(|item| {
            if cursor.prefix.range_end == key.len() {
                // If the cursor's range ends where the key ends, we have a hit.
                Some(TrieSearch::ExactHit(item.clone()))
            } else {
                // Otherwise the cursor's range is greater than the key's length which means the
                // key is a prefix of the match.
                None
            }
        })
    }

    /// Whether the node is a root node.
    pub fn is_root(&self) -> bool {
        matches!(self.prefix.key, TrieKey::Root)
    }

    fn new_leaf(new_item: T) -> Self {
        Self {
            child_nodes: HashMap::default(),
            // We have to include leaf nodes as descendants of themselves, because
            // leaf nodes can be returned in `TrieSearch::LongestPrefixNode` results after
            // eliminating one-way branching.
            descendants: vec![new_item.clone()],
            match_: Some(new_item.clone()),
            prefix: TriePrefix {
                range_end: new_item.key().len(),
                key: TrieKey::Key(new_item),
            },
        }
    }

    /// Create a new split node with the split prefix.
    /// Fill it by calling `Self::fill_split_node` on it after swapping it with
    /// the node to split.
    fn new_split_node(split_index: usize, new_item: T) -> Self {
        BytecodeTrie {
            prefix: TriePrefix {
                range_end: split_index,
                key: TrieKey::Key(new_item),
            },

            // These be filled in by calling `Self::fill_split_node`
            child_nodes: HashMap::default(),
            descendants: Vec::default(),
            match_: None,
        }
    }

    /// Fill the split node from the node with the node
    /// and the new key as children. If the new key is shorter than the
    /// node's prefix, the new split node will be a match for the new
    /// key. Panics if `split_index > new_key.key().len()`
    fn fill_split_node(&mut self, node_to_split: BytecodeTrie<T>, new_item: T) {
        // We use the descendants to keep track of insertion order, so it's
        // important to preserve that order here
        let mut descendants = Vec::with_capacity(node_to_split.descendants.len() + 1);
        descendants.extend(node_to_split.descendants.iter().cloned());
        descendants.push(new_item.clone());

        let split_index = self.prefix.range_end;

        // Add occupied node as child
        self.child_nodes.insert(
            node_to_split.prefix.key.key()[split_index],
            Box::new(node_to_split),
        );

        match split_index.cmp(&new_item.key().len()) {
            Ordering::Less => {
                // If the new key is longer than the end of split prefix, add it as a child node
                self.child_nodes.insert(
                    new_item.key()[split_index],
                    Box::new(Self::new_leaf(new_item)),
                );
            }
            Ordering::Equal => {
                // Otherwise it's an exact match for the split node
                self.match_ = Some(new_item);
            }
            Ordering::Greater => {
                // If the split index is greater than the length of the key, this function was
                // called with the wrong arguments due to a bug.
                panic!("split index is greater than new key length")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TriePrefix<T> {
    /// `key[..range_end]` represents this prefix
    range_end: usize,
    key: TrieKey<T>,
}

impl<T: TrieKeyTrait> TriePrefix<T> {
    fn new_root() -> Self {
        Self {
            // Root node never needs to be split
            range_end: 0,
            key: TrieKey::Root,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TrieKey<T> {
    Root,
    Key(T),
}

impl<T: TrieKeyTrait> TrieKeyTrait for TrieKey<T> {
    fn key(&self) -> &[u8] {
        match self {
            TrieKey::Root => &[],
            TrieKey::Key(key) => key.key(),
        }
    }
}

impl TrieKeyTrait for Arc<ContractMetadata> {
    fn key(&self) -> &[u8] {
        &self.normalized_code
    }
}

#[cfg(test)]
mod tests {
    use crate::bytecode_trie::{BytecodeTrie, TrieKeyTrait, TrieSearch};

    impl<T: TrieKeyTrait> BytecodeTrie<T> {
        /// The number of nodes in this (sub)trie. Useful for testing.
        fn node_count(&self) -> usize {
            1 + self
                .child_nodes
                .values()
                .map(|node| node.node_count())
                .sum::<usize>()
        }
    }

    impl TrieKeyTrait for &'static str {
        fn key(&self) -> &[u8] {
            self.as_bytes()
        }
    }

    fn assert_prefix_node(
        result: Option<TrieSearch<'_, &'static str>>,
        expected_prefix: &str,
        should_match: bool,
    ) {
        let Some(TrieSearch::LongestPrefixNode {
            node,
            diff_index: _,
            match_,
        }) = result
        else {
            assert!(result.is_some(), "received None");
            panic!("received exact hit");
        };
        assert_eq!(should_match, match_.is_some());
        assert_eq!(
            &node.prefix.key.key()[..node.prefix.range_end],
            expected_prefix.as_bytes()
        );
    }

    #[test]
    fn test_one_key_exact_match() {
        let mut trie = BytecodeTrie::<&'static str>::new_root();
        trie.add("hello");
        assert_eq!(
            trie.search(b"hello", 0),
            Some(TrieSearch::ExactHit("hello"))
        );
        assert_eq!(trie.node_count(), 2);
    }

    #[test]
    fn test_empty_trie() {
        let trie = BytecodeTrie::<&'static str>::new_root();
        assert_eq!(trie.search(b"hello", 0), None);
        assert_eq!(trie.node_count(), 1);
    }

    #[test]
    fn test_empty_trie_empty_key() {
        let trie = BytecodeTrie::<&'static str>::new_root();
        assert_eq!(trie.search(&[], 0), None);
    }

    #[test]
    fn test_empty_some_trie_empty_key() {
        let mut trie = BytecodeTrie::<&'static str>::new_root();
        trie.add("hello");
        assert_eq!(trie.search(&[], 0), None);
    }

    #[test]
    fn test_one_key_prefix() {
        let mut trie = BytecodeTrie::<&'static str>::new_root();
        trie.add("hello");
        assert_prefix_node(trie.search(b"hellos", 0), "hello", true);
        assert_eq!(trie.search(b"hel", 0), None);
        assert_eq!(trie.node_count(), 2);
    }

    #[test]
    fn test_not_found() {
        let mut trie = BytecodeTrie::<&'static str>::new_root();
        trie.add("hello");
        assert_eq!(trie.search(b"foo", 0), None);
    }

    #[test]
    fn test_duplicate_key() {
        let mut trie = BytecodeTrie::<&'static str>::new_root();
        trie.add("hello");
        trie.add("hello");
        assert_eq!(
            trie.search(b"hello", 0),
            Some(TrieSearch::ExactHit("hello"))
        );
        assert_eq!(trie.node_count(), 2);
    }

    #[test]
    fn test_two_keys_that_are_prefix() {
        let mut trie = BytecodeTrie::<&'static str>::new_root();
        trie.add("hello");
        trie.add("hellos");
        assert_eq!(
            trie.search(b"hello", 0),
            Some(TrieSearch::ExactHit("hello"))
        );
        assert_eq!(
            trie.search(b"hellos", 0),
            Some(TrieSearch::ExactHit("hellos"))
        );
        assert_eq!(trie.node_count(), 3);
    }

    #[test]
    fn test_root_multiple_children() {
        let mut trie = BytecodeTrie::<&'static str>::new_root();
        trie.add("foo");
        trie.add("bar");
        assert_eq!(trie.search(b"foo", 0), Some(TrieSearch::ExactHit("foo")));
        assert_eq!(trie.search(b"bar", 0), Some(TrieSearch::ExactHit("bar")));
        assert_eq!(trie.node_count(), 3);
    }

    #[test]
    fn test_non_trivial() {
        // Based on Sedgewick, Algorithms
        let mut trie = BytecodeTrie::<&'static str>::new_root();
        let words = vec!["shell", "sheep", "shells", "shellfish", "ship"];

        for word in &words {
            trie.add(word);
        }

        // + 3 = root node + two split nodes ("sh" and "she")
        assert_eq!(trie.node_count(), words.len() + 3);

        for word in &words {
            assert_eq!(
                trie.search(word.as_bytes(), 0),
                Some(TrieSearch::ExactHit(*word))
            );
        }

        // Expected prefix is leaf
        assert_prefix_node(trie.search(b"sheepherder", 0), "sheep", true);
        // Expected prefix is split node with match
        assert_prefix_node(trie.search(b"shelly", 0), "shell", true);
        // Expected prefix is split node without match
        assert_prefix_node(trie.search(b"sharp", 0), "sh", false);

        // Split nodes without match shouldn't match
        assert_eq!(trie.search(b"sh", 0), None);
        assert_eq!(trie.search(b"she", 0), None);

        // Prefix contained in trie without node shouldn't match
        assert_eq!(trie.search(b"shee", 0), None);
    }
}
