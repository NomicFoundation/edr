use std::{collections::HashMap, rc::Rc};

use edr_eth::Address;
use edr_evm::hex;
use napi::{
    bindgen_prelude::{ClassInstance, Uint8Array},
    Either, Env,
};
use napi_derive::napi;

use super::{
    model::{Bytecode, ImmutableReference},
    opcodes::Opcode,
};
use crate::utils::ClassInstanceRef;

/// This class represent a somewhat special Trie of bytecodes.
///
/// What makes it special is that every node has a set of all of its descendants
/// and its depth.
#[derive(Clone)]
pub struct BytecodeTrie {
    child_nodes: HashMap<u8, Box<BytecodeTrie>>,
    descendants: Vec<Rc<ClassInstanceRef<Bytecode>>>,
    match_: Option<Rc<ClassInstanceRef<Bytecode>>>,
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

    pub fn add(&mut self, bytecode: ClassInstance<Bytecode>, env: Env) -> napi::Result<()> {
        let bytecode = Rc::new(ClassInstanceRef::from_obj(bytecode, env)?);

        let mut cursor = self;

        let bytecode_normalized_code = &bytecode.borrow(env)?.normalized_code;
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
        code: &Uint8Array,
        current_code_byte: u32,
    ) -> Option<Either<Rc<ClassInstanceRef<Bytecode>>, &Self>> {
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
fn is_matching_metadata(code: Uint8Array, last_byte: u32) -> bool {
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

#[napi]
pub struct ContractsIdentifier {
    trie: BytecodeTrie,
    cache: HashMap<String, Rc<ClassInstanceRef<Bytecode>>>,
    enable_cache: bool,
}

#[napi]
impl ContractsIdentifier {
    #[napi(constructor)]
    pub fn new(enable_cache: Option<bool>) -> ContractsIdentifier {
        let enable_cache = enable_cache.unwrap_or(true);

        ContractsIdentifier {
            trie: BytecodeTrie::new(None),
            cache: HashMap::new(),
            enable_cache,
        }
    }

    #[napi]
    pub fn add_bytecode(
        &mut self,
        bytecode: ClassInstance<Bytecode>,
        env: Env,
    ) -> napi::Result<()> {
        self.trie.add(bytecode, env)?;
        self.cache.clear();

        Ok(())
    }

    fn search_bytecode(
        &mut self,
        is_create: bool,
        code: Uint8Array,
        normalize_libraries: Option<bool>,
        trie: Option<&BytecodeTrie>,
        first_byte_to_search: Option<u32>,
        env: Env,
    ) -> napi::Result<Option<Rc<ClassInstanceRef<Bytecode>>>> {
        let normalize_libraries = normalize_libraries.unwrap_or(true);
        let first_byte_to_search = first_byte_to_search.unwrap_or(0);
        let trie = trie.unwrap_or(&self.trie);

        Self::search_bytecode_inner(
            is_create,
            code,
            normalize_libraries,
            trie,
            first_byte_to_search,
            env,
        )
    }

    fn search_bytecode_inner(
        is_create: bool,
        code: Uint8Array,
        normalize_libraries: bool,
        trie: &BytecodeTrie,
        first_byte_to_search: u32,
        env: Env,
    ) -> napi::Result<Option<Rc<ClassInstanceRef<Bytecode>>>> {
        let search_result = match trie.search(&code, first_byte_to_search) {
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
            Some(bytecode) if is_create && bytecode.borrow(env)?.is_deployment => {
                return Ok(Some(bytecode.clone()));
            }
            _ => {}
        };

        if normalize_libraries {
            for bytecode_with_libraries in &search_result.descendants {
                let bytecode_with_libraries = bytecode_with_libraries.borrow(env)?;

                if bytecode_with_libraries.library_address_positions.is_empty()
                    && bytecode_with_libraries.immutable_references.is_empty()
                {
                    continue;
                }

                let mut normalized_code = code.clone();
                // zero out addresses
                for pos in &bytecode_with_libraries.library_address_positions {
                    normalized_code[*pos as usize..][..Address::len_bytes()].fill(0);
                }
                // zero out slices
                for ImmutableReference { start, length } in
                    &bytecode_with_libraries.immutable_references
                {
                    normalized_code[*start as usize..][..*length as usize].fill(0);
                }

                let normalized_result = Self::search_bytecode_inner(
                    is_create,
                    normalized_code,
                    false,
                    search_result,
                    search_result.depth.map_or(0, |depth| depth + 1),
                    env,
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
        code: Uint8Array,
        is_create: bool,
        env: Env,
    ) -> napi::Result<Option<Rc<ClassInstanceRef<Bytecode>>>> {
        let mut normalized_code = code.clone();
        normalize_library_runtime_bytecode_if_necessary(&mut normalized_code);

        let normalized_code_hex = hex::encode(normalized_code.as_ref());
        if self.enable_cache {
            let cached = self.cache.get(&normalized_code_hex);

            if let Some(cached) = cached {
                return Ok(Some(cached.clone()));
            }
        }

        let result = self.search_bytecode(is_create, normalized_code, None, None, None, env)?;

        if self.enable_cache {
            if let Some(result) = &result {
                self.cache.insert(normalized_code_hex, result.clone());
            }
        }

        Ok(result)
    }
}

fn normalize_library_runtime_bytecode_if_necessary(code: &mut Uint8Array) {
    // Libraries' protection normalization:
    // Solidity 0.4.20 introduced a protection to prevent libraries from being
    // called directly. This is done by modifying the code on deployment, and
    // hard-coding the contract address. The first instruction is a PUSH20 of
    // the address, which we zero-out as a way of normalizing it. Note that it's
    // also zeroed-out in the compiler output.
    if code.first().copied() == Some(Opcode::PUSH20 as u8) {
        code[1..][..Address::len_bytes()].fill(0);
    }
}
