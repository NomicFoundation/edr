use std::{collections::HashMap, sync::Arc};

use crate::build_model::ContractMetadata;

/// The result of searching for a bytecode in a [`BytecodeTrie`].
pub enum TrieSearch<'a> {
    /// An exact match was found.
    ExactHit(Arc<ContractMetadata>),
    /// No exact match found; a node with the longest prefix is returned.
    LongestPrefixNode(&'a BytecodeTrie),
}

/// This class represent a somewhat special Trie of bytecodes.
///
/// What makes it special is that every node has a set of all of its descendants
/// and its depth.
#[derive(Debug, Clone)]
pub struct BytecodeTrie {
    pub descendants: Vec<Arc<ContractMetadata>>,
    pub match_: Option<Arc<ContractMetadata>>,
    pub depth: Option<u32>,
    child_nodes: HashMap<u8, Box<BytecodeTrie>>,
}

impl BytecodeTrie {
    pub fn new(depth: Option<u32>) -> BytecodeTrie {
        BytecodeTrie {
            child_nodes: HashMap::new(),
            descendants: Vec::new(),
            match_: None,
            depth,
        }
    }

    pub fn add(&mut self, bytecode: Arc<ContractMetadata>) {
        let mut cursor = self;

        let bytecode_normalized_code = &bytecode.normalized_code;
        for (index, byte) in bytecode_normalized_code.iter().enumerate() {
            cursor.descendants.push(bytecode.clone());

            let node = cursor
                .child_nodes
                .entry(*byte)
                .or_insert_with(|| Box::new(BytecodeTrie::new(Some(index as u32))));

            cursor = node;
        }

        // If multiple contracts with the exact same bytecode are added we keep the last
        // of them. Note that this includes the metadata hash, so the chances of
        // happening are pretty remote, except in super artificial cases that we
        // have in our test suite.
        cursor.match_ = Some(bytecode.clone());
    }

    /// Searches for a bytecode. If it's an exact match, it is returned. If
    /// there's no match, but a prefix of the code is found in the trie, the
    /// node of the longest prefix is returned. If the entire code is
    /// covered by the trie, and there's no match, we return None.
    pub fn search(&self, code: &[u8], current_code_byte: u32) -> Option<TrieSearch<'_>> {
        if current_code_byte > code.len() as u32 {
            return None;
        }

        let mut cursor = self;

        for byte in code.iter().skip(current_code_byte as usize) {
            let child_node = cursor.child_nodes.get(byte);

            if let Some(node) = child_node {
                cursor = node;
            } else {
                return Some(TrieSearch::LongestPrefixNode(cursor));
            }
        }

        cursor
            .match_
            .as_ref()
            .map(|bytecode| TrieSearch::ExactHit(bytecode.clone()))
    }
}
