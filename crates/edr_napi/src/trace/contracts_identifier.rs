use std::{collections::HashMap, rc::Rc};

use edr_eth::Address;
use edr_evm::hex;
use napi::{
    bindgen_prelude::{ClassInstance, Either3, Object, Uint8Array, Undefined},
    Either, Env, JsObject,
};
use napi_derive::napi;

use super::{
    model::{Bytecode, ImmutableReference},
    opcodes::Opcode,
};
use crate::utils::ClassInstanceRef;

// TODO: Remove me once we do not need to surface this to JS
/// A cursor that differentiates between the Rust root trie and the JS leaf
/// trie.
enum BytecodeTrieCursor<'a> {
    Root(&'a mut BytecodeTrie),
    Leaf(Rc<ClassInstanceRef<BytecodeTrie>>),
}

enum BytecodeTrieCursorRef<'a> {
    Root(&'a BytecodeTrie),
    Leaf(Rc<ClassInstanceRef<BytecodeTrie>>),
}

/// This class represent a somewhat special Trie of bytecodes.
///
/// What makes it special is that every node has a set of all of its descendants
/// and its depth.
#[napi]
pub struct BytecodeTrie {
    child_nodes: HashMap<u8, Rc<ClassInstanceRef<BytecodeTrie>>>,
    descendants: Vec<Rc<ClassInstanceRef<Bytecode>>>,
    match_: Option<Rc<ClassInstanceRef<Bytecode>>>,
    #[napi(readonly)]
    pub depth: u32,
}

#[napi]
impl BytecodeTrie {
    #[napi(constructor)]
    pub fn new(depth: u32) -> BytecodeTrie {
        BytecodeTrie {
            child_nodes: HashMap::new(),
            descendants: Vec::new(),
            match_: None,
            depth,
        }
    }

    #[napi(getter, ts_return_type = "Array<Bytecode>")]
    pub fn descendants(&self, env: Env) -> napi::Result<Vec<Object>> {
        self.descendants
            .iter()
            .map(|descendant| descendant.as_object(env))
            .collect()
    }

    #[napi(getter, js_name = "match", ts_return_type = "Bytecode | undefined")]
    pub fn match_(&self, env: Env) -> napi::Result<Either<Object, Undefined>> {
        match &self.match_ {
            Some(match_) => match_.as_object(env).map(Either::A),
            None => Ok(Either::B(())),
        }
    }

    #[napi]
    pub fn add(&mut self, bytecode: ClassInstance<Bytecode>, env: Env) -> napi::Result<()> {
        let bytecode = Rc::new(ClassInstanceRef::from_obj(bytecode, env)?);

        // TODO: Get rid of the cursor once we don't need to differentiate between Rust
        // and JS objects
        let mut cursor = BytecodeTrieCursor::Root(self);

        let bytecode_normalized_code = &bytecode.borrow(env)?.normalized_code;
        for (index, byte) in bytecode_normalized_code.iter().enumerate() {
            // Add a descendant
            match &mut cursor {
                BytecodeTrieCursor::Root(trie) => trie.descendants.push(bytecode.clone()),
                BytecodeTrieCursor::Leaf(trie) => {
                    trie.borrow_mut(env)?.descendants.push(bytecode.clone());
                }
            }

            // Get or insert the child node
            let node = {
                let child_nodes = match &mut cursor {
                    BytecodeTrieCursor::Root(trie) => &mut trie.child_nodes,
                    BytecodeTrieCursor::Leaf(trie) => &mut trie.borrow_mut(env)?.child_nodes,
                };

                match child_nodes.entry(*byte) {
                    std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let inst = BytecodeTrie::new(index as u32).into_instance(env)?;
                        let inst_ref = Rc::new(ClassInstanceRef::from_obj(inst, env)?);
                        entry.insert(inst_ref)
                    }
                }
                .clone()
            };

            cursor = BytecodeTrieCursor::Leaf(node);
        }

        // If multiple contracts with the exact same bytecode are added we keep the last
        // of them. Note that this includes the metadata hash, so the chances of
        // happening are pretty remote, except in super artificial cases that we
        // have in our test suite.
        let match_ = match &mut cursor {
            BytecodeTrieCursor::Root(trie) => &mut trie.match_,
            BytecodeTrieCursor::Leaf(trie) => &mut trie.borrow_mut(env)?.match_,
        };
        *match_ = Some(bytecode.clone());

        Ok(())
    }

    /// Searches for a bytecode. If it's an exact match, it is returned. If
    /// there's no match, but a prefix of the code is found in the trie, the
    /// node of the longest prefix is returned. If the entire code is
    /// covered by the trie, and there's no match, we return undefined.
    #[napi(ts_return_type = "Bytecode | BytecodeTrie | undefined")]
    pub fn search(
        &mut self,
        code: Uint8Array,
        current_code_byte: u32,
        env: Env,
    ) -> napi::Result<Either<JsObject, Undefined>> {
        if current_code_byte > code.len() as u32 {
            return Ok(Either::B(()));
        }

        let mut cursor = BytecodeTrieCursor::Root(self);
        for byte in code.iter().skip(current_code_byte as usize) {
            let child_node = match &mut cursor {
                BytecodeTrieCursor::Root(trie) => trie.child_nodes.get(byte).cloned(),
                BytecodeTrieCursor::Leaf(trie) => {
                    trie.borrow_mut(env)?.child_nodes.get(byte).cloned()
                }
            };

            if let Some(node) = child_node {
                cursor = BytecodeTrieCursor::Leaf(node);
            } else {
                return match &mut cursor {
                    BytecodeTrieCursor::Root(..) => Ok(Either::B(())),
                    BytecodeTrieCursor::Leaf(trie) => trie.as_object(env).map(Either::A),
                };
            }
        }

        let match_ = match cursor {
            BytecodeTrieCursor::Root(trie) => &trie.match_,
            BytecodeTrieCursor::Leaf(ref trie) => &trie.borrow_mut(env)?.match_,
        };
        match match_ {
            Some(bytecode) => Ok(Either::A(bytecode.as_object(env)?)),
            None => Ok(Either::B(())),
        }
    }

    pub fn search_inner(
        &self,
        code: &Uint8Array,
        current_code_byte: u32,
        env: Env,
    ) -> napi::Result<
        Either3<Rc<ClassInstanceRef<Bytecode>>, Rc<ClassInstanceRef<BytecodeTrie>>, Undefined>,
    > {
        if current_code_byte > code.len() as u32 {
            return Ok(Either3::C(()));
        }

        let mut cursor = BytecodeTrieCursorRef::Root(self);
        for byte in code.iter().skip(current_code_byte as usize) {
            let child_node = match &mut cursor {
                BytecodeTrieCursorRef::Root(trie) => trie.child_nodes.get(byte).cloned(),
                BytecodeTrieCursorRef::Leaf(trie) => {
                    trie.borrow_mut(env)?.child_nodes.get(byte).cloned()
                }
            };

            if let Some(node) = child_node {
                cursor = BytecodeTrieCursorRef::Leaf(node);
            } else {
                return Ok(match &mut cursor {
                    BytecodeTrieCursorRef::Root(..) => Either3::C(()),
                    BytecodeTrieCursorRef::Leaf(trie) => Either3::B(trie.clone()),
                });
            }
        }

        let match_ = match cursor {
            BytecodeTrieCursorRef::Root(trie) => &trie.match_,
            BytecodeTrieCursorRef::Leaf(ref trie) => &trie.borrow_mut(env)?.match_,
        };
        match match_ {
            Some(bytecode) => Ok(Either3::A(bytecode.clone())),
            None => Ok(Either3::C(())),
        }
    }
}

/// Returns true if the lastByte is placed right when the metadata starts or
/// after it.
#[napi]
pub fn is_matching_metadata(code: Uint8Array, last_byte: u32) -> bool {
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
    trie: Rc<ClassInstanceRef<BytecodeTrie>>,
    cache: HashMap<String, Rc<ClassInstanceRef<Bytecode>>>,
    enable_cache: bool,
}

#[napi]
impl ContractsIdentifier {
    #[napi(constructor)]
    pub fn new(enable_cache: Option<bool>, env: Env) -> ContractsIdentifier {
        let enable_cache = enable_cache.unwrap_or(true);

        // TODO: This shouldn't be necessary once we do not need to call it via JS in
        // `fn search` TODO: Does it really matter that it's -1 in the JS
        // implementation?
        let trie = BytecodeTrie::new(0).into_instance(env).unwrap();
        let trie = Rc::new(ClassInstanceRef::from_obj(trie, env).unwrap());

        ContractsIdentifier {
            trie,
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
        self.trie.borrow_mut(env)?.add(bytecode, env)?;
        self.cache.clear();

        Ok(())
    }

    fn search_bytecode(
        &mut self,
        is_create: bool,
        code: Uint8Array,
        normalize_libraries: Option<bool>,
        trie: Option<ClassInstance<BytecodeTrie>>,
        first_byte_to_search: Option<u32>,
        env: Env,
    ) -> napi::Result<Option<Rc<ClassInstanceRef<Bytecode>>>> {
        let normalize_libraries = normalize_libraries.unwrap_or(true);
        let first_byte_to_search = first_byte_to_search.unwrap_or(0);
        let trie = trie
            .map(|trie| ClassInstanceRef::from_obj(trie, env))
            .transpose()?
            .map_or_else(|| self.trie.clone(), Rc::new);

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
        trie: Rc<ClassInstanceRef<BytecodeTrie>>,
        first_byte_to_search: u32,
        env: Env,
    ) -> napi::Result<Option<Rc<ClassInstanceRef<Bytecode>>>> {
        let search_result = trie
            .borrow(env)?
            .search_inner(&code, first_byte_to_search, env)?;

        let search_result = match search_result {
            Either3::A(bytecode) => return Ok(Some(bytecode.clone())),
            Either3::B(trie) => trie,
            Either3::C(()) => return Ok(None),
        };

        let search_result_ref = search_result.borrow(env)?;

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
        // exactly matched the searchResult (sub)trie that we got.
        match &search_result_ref.match_ {
            Some(bytecode) if is_create && bytecode.borrow(env)?.is_deployment => {
                return Ok(Some(bytecode.clone()));
            }
            _ => {}
        };

        if normalize_libraries {
            for bytecode_with_libraries in &search_result_ref.descendants {
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
                    search_result.clone(),
                    search_result_ref.depth + 1,
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
        if is_matching_metadata(code, search_result_ref.depth)
            && !search_result_ref.descendants.is_empty()
        {
            return Ok(Some(
                search_result_ref.descendants[search_result_ref.descendants.len() - 1].clone(),
            ));
        }

        Ok(None)
    }

    #[napi(ts_return_type = "Bytecode | undefined")]
    pub fn get_bytecode_for_call(
        &mut self,
        code: Uint8Array,
        is_create: bool,
        env: Env,
    ) -> napi::Result<Either<JsObject, Undefined>> {
        let mut normalized_code = code.clone();
        normalize_library_runtime_bytecode_if_necessary(&mut normalized_code);

        let normalized_code_hex = hex::encode(normalized_code.as_ref());
        if self.enable_cache {
            let cached = self.cache.get(&normalized_code_hex);

            if let Some(cached) = cached {
                return Ok(Either::A(cached.as_object(env)?));
            }
        }

        let result = self.search_bytecode(is_create, normalized_code, None, None, None, env)?;

        if self.enable_cache {
            if let Some(result) = &result {
                self.cache.insert(normalized_code_hex, result.clone());
            }
        }

        match result {
            Some(bytecode) => Ok(Either::A(bytecode.as_object(env)?)),
            None => Ok(Either::B(())),
        }
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
