use std::{collections::HashMap, rc::Rc};

use napi::{
    bindgen_prelude::{ClassInstance, Object, Uint8Array, Undefined},
    Either, Env, JsObject,
};
use napi_derive::napi;

use super::model::Bytecode;
use crate::utils::ClassInstanceRef;

// TODO: Remove me once we do not need to surface this to JS
/// A cursor that differentiates between the Rust root trie and the JS leaf
/// trie.
enum BytecodeTrieCursor<'a> {
    Root(&'a mut BytecodeTrie),
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
}
