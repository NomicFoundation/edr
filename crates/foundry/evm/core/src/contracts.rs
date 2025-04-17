//! Commonly used contract types and functions.

use std::collections::BTreeMap;

use alloy_json_abi::JsonAbi;
use alloy_primitives::Address;
use edr_solidity::artifacts::{ArtifactId, ContractData};
use eyre::Result;
use foundry_compilers::{
    artifacts::{
        CompactBytecode, CompactContractBytecode, CompactContractBytecodeCow, ContractBytecodeSome,
    },
    Artifact,
};

type ArtifactWithContractRef<'a> = (&'a ArtifactId, &'a ContractData);

/// Wrapper type that maps an artifact to a contract ABI and bytecode.
#[derive(Clone, Default, Debug)]
pub struct ContractsByArtifact(BTreeMap<ArtifactId, ContractData>);

impl ContractsByArtifact {
    /// Creates a new instance by collecting all artifacts with present bytecode
    /// from an iterator. Excludes artifacts without bytecode.
    pub fn new<'a>(
        artifacts: impl IntoIterator<Item = (ArtifactId, CompactContractBytecodeCow<'a>)>,
    ) -> Self {
        let map = artifacts
            .into_iter()
            .filter_map(|(id, artifact)| {
                let CompactContractBytecode {
                    abi,
                    bytecode,
                    deployed_bytecode,
                } = artifact.into_contract_bytecode();
                Some((
                    id,
                    ContractData {
                        abi: abi?,
                        bytecode: bytecode.and_then(CompactBytecode::into_bytes),
                        deployed_bytecode: deployed_bytecode.and_then(|deployed_bytecode| {
                            deployed_bytecode
                                .bytecode
                                .and_then(CompactBytecode::into_bytes)
                        }),
                    },
                ))
            })
            .collect();
        Self(map)
    }

    /// Returns an iterator over all ids and contracts.
    pub fn iter(&self) -> impl Iterator<Item = ArtifactWithContractRef<'_>> {
        self.0.iter()
    }

    /// Get a contract by its id.
    pub fn get(&self, id: &ArtifactId) -> Option<&ContractData> {
        self.0.get(id)
    }

    /// Returns the number of contracts.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if there are no contracts.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over the contracts.
    pub fn values(&self) -> impl Iterator<Item = &ContractData> {
        self.0.values()
    }

    /// Finds a contract which has a similar deployed bytecode as `code`.
    pub fn find_by_deployed_code(&self, code: &[u8]) -> Option<ArtifactWithContractRef<'_>> {
        self.iter().find(|(_, contract)| {
            if let Some(deployed_bytecode) = &contract.deployed_bytecode {
                bytecode_diff_score(deployed_bytecode.as_ref(), code) <= 0.1
            } else {
                false
            }
        })
    }

    /// Finds a contract which has the same contract name or identifier as `id`.
    /// If more than one is found, return error.
    pub fn find_by_name_or_identifier(
        &self,
        id: &str,
    ) -> Result<Option<ArtifactWithContractRef<'_>>> {
        let contracts = self
            .iter()
            .filter(|(artifact, _)| artifact.name == id || artifact.identifier() == id)
            .collect::<Vec<_>>();

        if contracts.len() > 1 {
            eyre::bail!("{id} has more than one implementation.");
        }

        Ok(contracts.first().cloned())
    }

    /// Finds abi for contract which has the same contract name or identifier as
    /// `id`.
    pub fn find_abi_by_name_or_identifier(&self, id: &str) -> Option<JsonAbi> {
        self.iter()
            .find(|(artifact, _)| {
                artifact.name.split(".").next().unwrap() == id || artifact.identifier() == id
            })
            .map(|(_, contract)| contract.abi.clone())
    }
}

impl From<BTreeMap<ArtifactId, ContractData>> for ContractsByArtifact {
    fn from(value: BTreeMap<ArtifactId, ContractData>) -> Self {
        Self(value)
    }
}

/// Wrapper type that maps an address to a contract identifier and contract ABI.
pub type ContractsByAddress = BTreeMap<Address, (String, JsonAbi)>;

/// Very simple fuzzy matching of contract bytecode.
///
/// Returns a value between `0.0` (identical) and `1.0` (completely different).
pub fn bytecode_diff_score<'a>(mut a: &'a [u8], mut b: &'a [u8]) -> f64 {
    // Make sure `a` is the longer one.
    if a.len() < b.len() {
        std::mem::swap(&mut a, &mut b);
    }

    // Account for different lengths.
    let mut n_different_bytes = a.len() - b.len();

    // If the difference is more than 32 bytes and more than 10% of the total
    // length, we assume the bytecodes are completely different.
    // This is a simple heuristic to avoid checking every byte when the lengths are
    // very different. 32 is chosen to be a reasonable minimum as it's the size
    // of metadata hashes and one EVM word.
    if n_different_bytes > 32 && n_different_bytes * 10 > a.len() {
        return 1.0;
    }

    // Count different bytes.
    // SAFETY: `a` is longer than `b`.
    n_different_bytes += unsafe { count_different_bytes(a, b) };

    n_different_bytes as f64 / a.len() as f64
}

/// Returns the amount of different bytes between two slices.
///
/// # Safety
///
/// `a` must be at least as long as `b`.
unsafe fn count_different_bytes(a: &[u8], b: &[u8]) -> usize {
    // This could've been written as `std::iter::zip(a, b).filter(|(x, y)| x !=
    // y).count()`, however this function is very hot, and has been written to
    // be as primitive as possible for lower optimization levels.

    let a_ptr = a.as_ptr();
    let b_ptr = b.as_ptr();
    let len = b.len();

    let mut sum = 0;
    let mut i = 0;
    while i < len {
        // SAFETY: `a` is at least as long as `b`, and `i` is in bound of `b`.
        sum += usize::from(unsafe { *a_ptr.add(i) != *b_ptr.add(i) });
        i += 1;
    }
    sum
}

/// Helper function to convert `CompactContractBytecode` ~>
/// `ContractBytecodeSome`
pub fn compact_to_contract(contract: CompactContractBytecode) -> Result<ContractBytecodeSome> {
    Ok(ContractBytecodeSome {
        abi: contract.abi.ok_or_else(|| eyre::eyre!("No contract abi"))?,
        bytecode: contract
            .bytecode
            .ok_or_else(|| eyre::eyre!("No contract bytecode"))?
            .into(),
        deployed_bytecode: contract
            .deployed_bytecode
            .ok_or_else(|| eyre::eyre!("No contract deployed bytecode"))?
            .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytecode_diffing() {
        assert_eq!(bytecode_diff_score(b"a", b"a"), 0.0);
        assert_eq!(bytecode_diff_score(b"a", b"b"), 1.0);

        let a_100 = &b"a".repeat(100)[..];
        assert_eq!(bytecode_diff_score(a_100, &b"b".repeat(100)), 1.0);
        assert_eq!(bytecode_diff_score(a_100, &b"b".repeat(99)), 1.0);
        assert_eq!(bytecode_diff_score(a_100, &b"b".repeat(101)), 1.0);
        assert_eq!(bytecode_diff_score(a_100, &b"b".repeat(120)), 1.0);
        assert_eq!(bytecode_diff_score(a_100, &b"b".repeat(1000)), 1.0);

        let a_99 = &b"a".repeat(99)[..];
        assert!(bytecode_diff_score(a_100, a_99) <= 0.01);
    }
}
