//! Decodes `console.log` calls into the per-argument strings the corpus
//! expects.

use std::{collections::HashMap, sync::LazyLock};

use alloy_dyn_abi::{DynSolType, DynSolValue};
use anyhow::Context as _;
use edr_primitives::{hex, keccak256, Bytes};

/// Selector → argument types, read from the `console.sol` the corpus
/// imports, whose functions all forward `abi.encodeWithSignature("log(...)")`.
static SIGNATURES: LazyLock<HashMap<[u8; 4], Vec<DynSolType>>> = LazyLock::new(|| {
    let path = crate::corpus_dir().join("console.sol");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    source
        .split("abi.encodeWithSignature(\"")
        .skip(1)
        .map(|rest| {
            let signature = &rest[..rest.find('"').expect("unterminated signature")];
            let selector = *keccak256(signature.as_bytes())
                .first_chunk::<4>()
                .expect("keccak256 output is 32 bytes");
            let arguments = &signature["log(".len()..signature.len() - 1];
            let types = arguments
                .split(',')
                .filter(|argument| !argument.is_empty())
                .map(|argument| {
                    DynSolType::parse(argument)
                        .unwrap_or_else(|error| panic!("{signature}: {error}"))
                })
                .collect();
            (selector, types)
        })
        .collect()
});

/// Decodes one encoded `console.log` call into its formatted arguments.
pub(crate) fn decode(encoded: &Bytes) -> anyhow::Result<Vec<String>> {
    let (selector, arguments) = encoded
        .split_first_chunk::<4>()
        .context("console.log call shorter than a selector")?;
    let types = SIGNATURES
        .get(selector)
        .with_context(|| format!("unknown console.log selector 0x{}", hex::encode(selector)))?;
    let values = DynSolType::Tuple(types.clone()).abi_decode_params(arguments)?;
    let DynSolValue::Tuple(values) = values else {
        unreachable!("a tuple type decodes to a tuple value")
    };
    Ok(values.iter().map(format).collect())
}

fn format(value: &DynSolValue) -> String {
    match value {
        DynSolValue::Bool(value) => value.to_string(),
        DynSolValue::Int(value, _) => value.to_string(),
        DynSolValue::Uint(value, _) => value.to_string(),
        DynSolValue::Address(address) => address.to_checksum(None),
        DynSolValue::String(value) => value.clone(),
        DynSolValue::Bytes(bytes) => format!("0x{}", hex::encode(bytes)),
        DynSolValue::FixedBytes(word, size) => {
            format!("0x{}", hex::encode(word.get(..*size).unwrap_or_default()))
        }
        other => format!("{other:?}"),
    }
}
