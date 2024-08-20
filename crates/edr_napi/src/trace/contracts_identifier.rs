use std::{borrow::Cow, collections::HashMap, rc::Rc};

use edr_eth::Address;
use napi::Either;

use super::{
    model::{Bytecode, ImmutableReference},
    opcodes::Opcode,
};

/// This class represent a somewhat special Trie of bytecodes.
///
/// What makes it special is that every node has a set of all of its descendants
/// and its depth.
#[derive(Clone)]
pub struct BytecodeTrie {
    child_nodes: HashMap<u8, Box<BytecodeTrie>>,
    descendants: Vec<Rc<Bytecode>>,
    match_: Option<Rc<Bytecode>>,
    depth: Option<u32>,
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

    pub fn add(&mut self, bytecode: Rc<Bytecode>) -> napi::Result<()> {
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

        Ok(())
    }

    /// Searches for a bytecode. If it's an exact match, it is returned. If
    /// there's no match, but a prefix of the code is found in the trie, the
    /// node of the longest prefix is returned. If the entire code is
    /// covered by the trie, and there's no match, we return undefined.
    pub fn search(
        &self,
        code: &[u8],
        current_code_byte: u32,
    ) -> Option<Either<Rc<Bytecode>, &Self>> {
        if current_code_byte > code.len() as u32 {
            return None;
        }

        let mut cursor = self;

        for byte in code.iter().skip(current_code_byte as usize) {
            let child_node = cursor.child_nodes.get(byte);

            if let Some(node) = child_node {
                cursor = node;
            } else {
                return Some(Either::B(cursor));
            }
        }

        cursor
            .match_
            .as_ref()
            .map(|bytecode| Either::A(bytecode.clone()))
    }
}

/// Returns true if the lastByte is placed right when the metadata starts or
/// after it.
fn is_matching_metadata(code: &[u8], last_byte: u32) -> bool {
    let mut byte = 0;
    while byte < last_byte {
        let opcode = Opcode::from_repr(code[byte as usize]).unwrap();
        let next = code
            .get(byte as usize + 1)
            .and_then(|x| Opcode::from_repr(*x));

        if opcode == Opcode::REVERT && next == Some(Opcode::INVALID) {
            return true;
        }

        byte += u32::from(opcode.len());
    }

    false
}

pub struct ContractsIdentifier {
    trie: BytecodeTrie,
    cache: HashMap<Vec<u8>, Rc<Bytecode>>,
    enable_cache: bool,
}

impl ContractsIdentifier {
    pub fn new(enable_cache: Option<bool>) -> ContractsIdentifier {
        let enable_cache = enable_cache.unwrap_or(true);

        ContractsIdentifier {
            trie: BytecodeTrie::new(None),
            cache: HashMap::new(),
            enable_cache,
        }
    }

    pub(crate) fn add_bytecode(&mut self, bytecode: Rc<Bytecode>) -> napi::Result<()> {
        self.trie.add(bytecode)?;
        self.cache.clear();

        Ok(())
    }

    fn search_bytecode(
        &mut self,
        is_create: bool,
        code: &[u8],
        normalize_libraries: Option<bool>,
        trie: Option<&BytecodeTrie>,
        first_byte_to_search: Option<u32>,
    ) -> napi::Result<Option<Rc<Bytecode>>> {
        let normalize_libraries = normalize_libraries.unwrap_or(true);
        let first_byte_to_search = first_byte_to_search.unwrap_or(0);
        let trie = trie.unwrap_or(&self.trie);

        Self::search_bytecode_inner(
            is_create,
            code,
            normalize_libraries,
            trie,
            first_byte_to_search,
        )
    }

    fn search_bytecode_inner(
        is_create: bool,
        code: &[u8],
        normalize_libraries: bool,
        trie: &BytecodeTrie,
        first_byte_to_search: u32,
    ) -> napi::Result<Option<Rc<Bytecode>>> {
        let search_result = match trie.search(code, first_byte_to_search) {
            None => return Ok(None),
            Some(Either::A(bytecode)) => return Ok(Some(bytecode.clone())),
            Some(Either::B(trie)) => trie,
        };

        // Deployment messages have their abi-encoded arguments at the end of the
        // bytecode.
        //
        // We don't know how long those arguments are, as we don't know which contract
        // is being deployed, hence we don't know the signature of its
        // constructor.
        //
        // To make things even harder, we can't trust that the user actually passed the
        // right amount of arguments.
        //
        // Luckily, the chances of a complete deployment bytecode being the prefix of
        // another one are remote. For example, most of the time it ends with
        // its metadata hash, which will differ.
        //
        // We take advantage of this last observation, and just return the bytecode that
        // exactly matched the search_result (sub)trie that we got.
        match &search_result.match_ {
            Some(bytecode) if is_create && bytecode.is_deployment => {
                return Ok(Some(bytecode.clone()));
            }
            _ => {}
        };

        if normalize_libraries {
            for bytecode_with_libraries in &search_result.descendants {
                if bytecode_with_libraries.library_address_positions.is_empty()
                    && bytecode_with_libraries.immutable_references.is_empty()
                {
                    continue;
                }

                let mut normalized_code = code.to_vec();
                // zero out addresses
                for pos in &bytecode_with_libraries.library_address_positions {
                    if (*pos as usize + Address::len_bytes()) > normalized_code.len() {
                        continue;
                    }
                    normalized_code[*pos as usize..][..Address::len_bytes()].fill(0);
                }
                // zero out slices
                for ImmutableReference { start, length } in
                    &bytecode_with_libraries.immutable_references
                {
                    if *start as usize + *length as usize > normalized_code.len() {
                        continue;
                    }
                    normalized_code[*start as usize..][..*length as usize].fill(0);
                }

                let normalized_result = Self::search_bytecode_inner(
                    is_create,
                    &normalized_code,
                    false,
                    search_result,
                    search_result.depth.map_or(0, |depth| depth + 1),
                );

                if let Ok(Some(bytecode)) = normalized_result {
                    return Ok(Some(bytecode.clone()));
                };
            }
        }

        // If we got here we may still have the contract, but with a different metadata
        // hash.
        //
        // We check if we got to match the entire executable bytecode, and are just
        // stuck because of the metadata. If that's the case, we can assume that
        // any descendant will be a valid Bytecode, so we just choose the most
        // recently added one.
        //
        // The reason this works is because there's no chance that Solidity includes an
        // entire bytecode (i.e. with metadata), as a prefix of another one.
        if let Some(search_depth) = search_result.depth {
            if is_matching_metadata(code, search_depth) && !search_result.descendants.is_empty() {
                return Ok(Some(
                    search_result.descendants[search_result.descendants.len() - 1].clone(),
                ));
            }
        }

        Ok(None)
    }

    pub fn get_bytecode_for_call(
        &mut self,
        code: &[u8],
        is_create: bool,
    ) -> napi::Result<Option<Rc<Bytecode>>> {
        let normalized_code = normalize_library_runtime_bytecode_if_necessary(code);

        if self.enable_cache {
            let cached = self.cache.get(&*normalized_code);

            if let Some(cached) = cached {
                return Ok(Some(cached.clone()));
            }
        }

        let result = self.search_bytecode(is_create, &normalized_code, None, None, None)?;

        if self.enable_cache {
            if let Some(result) = &result {
                if !self.cache.contains_key(&*normalized_code) {
                    self.cache.insert(normalized_code.to_vec(), result.clone());
                }
            }
        }

        Ok(result)
    }
}

fn normalize_library_runtime_bytecode_if_necessary(code: &[u8]) -> Cow<'_, [u8]> {
    let mut code = Cow::Borrowed(code);

    // Libraries' protection normalization:
    // Solidity 0.4.20 introduced a protection to prevent libraries from being
    // called directly. This is done by modifying the code on deployment, and
    // hard-coding the contract address. The first instruction is a PUSH20 of
    // the address, which we zero-out as a way of normalizing it. Note that it's
    // also zeroed-out in the compiler output.
    if code.first().copied() == Some(Opcode::PUSH20 as u8) {
        code.to_mut()[1..][..Address::len_bytes()].fill(0);
    }

    code
}
