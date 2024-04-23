use revm_primitives::{EnvKzgSettings, B256, VERSIONED_HASH_VERSION_KZG};
use sha2::Digest;

use crate::transaction::Eip4844SignedTransaction;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eip4844PooledTransaction {
    payload: Eip4844SignedTransaction,
    blobs: Vec<c_kzg::Blob>,
    commitments: Vec<c_kzg::Bytes48>,
    proofs: Vec<c_kzg::Bytes48>,
}

#[derive(Debug, thiserror::Error)]
pub enum CreationError {
    #[error("Number of blobs ({actual}) does not match the payload's number of blob hashes ({expected}).")]
    BlobCount { expected: usize, actual: usize },
    #[error("The versioned hash of the commitment at index {idx} does not match the payload's blob hash. Expected: {expected}, actual: {actual}.")]
    InvalidCommitment {
        idx: usize,
        expected: B256,
        actual: B256,
    },
    #[error("Number of commitments ({actual}) does not match the payload's number of blob hashes ({expected}).")]
    CommitmentCount { expected: usize, actual: usize },
    #[error("An error occurred while verifying the blob KZG proof: {0}")]
    KzgProof(c_kzg::Error),
    #[error("Number of proofs ({actual}) does not match the payload's number of blob hashes ({expected}).")]
    ProofCount { expected: usize, actual: usize },
    #[error("The verification of the KZG proof failed.")]
    Unverified,
}

impl Eip4844PooledTransaction {
    /// Creates a new EIP-4844 pooled transaction, if the provided blobs,
    /// commitments, and proofs are valid.
    pub fn new(
        payload: Eip4844SignedTransaction,
        blobs: Vec<c_kzg::Blob>,
        commitments: Vec<c_kzg::Bytes48>,
        proofs: Vec<c_kzg::Bytes48>,
        settings: &c_kzg::KzgSettings,
    ) -> Result<Self, CreationError> {
        if payload.blob_hashes.len() != blobs.len() {
            return Err(CreationError::BlobCount {
                expected: payload.blob_hashes.len(),
                actual: blobs.len(),
            });
        }

        if payload.blob_hashes.len() != commitments.len() {
            return Err(CreationError::CommitmentCount {
                expected: payload.blob_hashes.len(),
                actual: commitments.len(),
            });
        }

        if payload.blob_hashes.len() != proofs.len() {
            return Err(CreationError::ProofCount {
                expected: payload.blob_hashes.len(),
                actual: proofs.len(),
            });
        }

        let verified = c_kzg::KzgProof::verify_blob_kzg_proof_batch(
            blobs.as_slice(),
            commitments.as_slice(),
            proofs.as_slice(),
            settings,
        )
        .map_err(CreationError::KzgProof)?;

        if !verified {
            return Err(CreationError::Unverified);
        }

        let invalid_blob_hash = payload
            .blob_hashes
            .iter()
            .zip(commitments.iter())
            .enumerate()
            .find_map(|(idx, (blob_hash, commitment))| {
                let mut commitment_hash = sha2::Sha256::digest(commitment.as_ref());
                commitment_hash[0] = VERSIONED_HASH_VERSION_KZG;

                if *blob_hash == *commitment_hash {
                    None
                } else {
                    Some((idx, *blob_hash, commitment_hash))
                }
            });

        if let Some((idx, expected, actual)) = invalid_blob_hash {
            return Err(CreationError::InvalidCommitment {
                idx,
                expected,
                actual: B256::from(actual.as_ref()),
            });
        }

        Ok(Self {
            payload,
            blobs,
            commitments,
            proofs,
        })
    }
}

#[repr(transparent)]
struct RlpBlob<'blob>(&'blob c_kzg::Blob);

impl<'blob> alloy_rlp::Encodable for RlpBlob<'blob> {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        out.put_slice(self.0.as_ref());
    }

    fn length(&self) -> usize {
        self.0.len()
    }
}

impl<'blob> From<&'blob c_kzg::Blob> for RlpBlob<'blob> {
    fn from(blob: &'blob c_kzg::Blob) -> Self {
        Self(blob)
    }
}

#[repr(transparent)]
struct RlpBytes48<'bytes>(&'bytes c_kzg::Bytes48);

impl<'bytes> alloy_rlp::Encodable for RlpBytes48<'bytes> {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        out.put_slice(self.0.as_ref());
    }

    fn length(&self) -> usize {
        self.0.len()
    }
}

impl<'bytes> From<&'bytes c_kzg::Bytes48> for RlpBytes48<'bytes> {
    fn from(bytes: &'bytes c_kzg::Bytes48) -> Self {
        Self(bytes)
    }
}

impl alloy_rlp::Decodable for Eip4844PooledTransaction {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let payload = Eip4844SignedTransaction::decode(buf)?;

        let blobs = Vec::<[u8; c_kzg::BYTES_PER_BLOB]>::decode(buf)?;
        let blobs = blobs.into_iter().map(c_kzg::Blob::from).collect::<Vec<_>>();

        let commitments = Vec::<[u8; c_kzg::BYTES_PER_COMMITMENT]>::decode(buf)?;
        let commitments = commitments
            .into_iter()
            .map(c_kzg::Bytes48::from)
            .collect::<Vec<_>>();

        let proofs = Vec::<[u8; c_kzg::BYTES_PER_PROOF]>::decode(buf)?;
        let proofs = proofs
            .into_iter()
            .map(c_kzg::Bytes48::from)
            .collect::<Vec<_>>();

        let settings = EnvKzgSettings::Default;
        Self::new(payload, blobs, commitments, proofs, settings.get()).map_err(|_error| {
            alloy_rlp::Error::Custom("Failed to RLP decode Eip4844PooledTransaction.")
        })
    }
}

impl alloy_rlp::Encodable for Eip4844PooledTransaction {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        self.payload.encode(out);
        alloy_rlp::encode_iter(self.blobs.iter().map(RlpBlob::from), out);
        alloy_rlp::encode_iter(self.commitments.iter().map(RlpBytes48::from), out);
        alloy_rlp::encode_iter(self.proofs.iter().map(RlpBytes48::from), out);
    }

    fn length(&self) -> usize {
        let blob_payload = self.blobs.len() * c_kzg::BYTES_PER_BLOB;
        let commitment_payload = self.commitments.len() * c_kzg::BYTES_PER_COMMITMENT;
        let proof_payload = self.proofs.len() * c_kzg::BYTES_PER_PROOF;

        self.payload.length()
            + blob_payload
            + alloy_rlp::length_of_length(blob_payload)
            + commitment_payload
            + alloy_rlp::length_of_length(commitment_payload)
            + proof_payload
            + alloy_rlp::length_of_length(proof_payload)
    }
}
