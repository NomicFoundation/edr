use std::str::FromStr as _;

use edr_eth::{
    eips::eip4844::ethereum_kzg_settings,
    rlp::{self, Decodable as _},
    Address, Blob, Bytes, Bytes48, B256,
};
use edr_test_utils::secret_key::secret_key_from_str;

/// Helper struct to modify the pooled transaction from the value in
/// `fixtures/eip4844.txt`. It reuses the secret key from `SECRET_KEYS[0]`.
pub struct BlobTransactionBuilder {
    request: edr_chain_l1::request::Eip4844,
    blobs: Vec<Blob>,
    commitments: Vec<Bytes48>,
    proofs: Vec<Bytes48>,
}

impl BlobTransactionBuilder {
    pub fn blob_hashes(&self) -> Vec<B256> {
        self.request.blob_hashes.clone()
    }

    pub fn build(self) -> edr_chain_l1::PooledTransaction {
        let secret_key =
            secret_key_from_str(edr_defaults::SECRET_KEYS[0]).expect("Invalid secret key");
        let signed_transaction = self
            .request
            .sign(&secret_key)
            .expect("Failed to sign transaction");

        let pooled_transaction = edr_chain_l1::pooled::Eip4844::new(
            signed_transaction,
            self.blobs,
            self.commitments,
            self.proofs,
            ethereum_kzg_settings(0),
        )
        .expect("Invalid blob transaction");

        edr_chain_l1::PooledTransaction::Eip4844(pooled_transaction)
    }

    pub fn build_raw(self) -> Bytes {
        rlp::encode(self.build()).into()
    }

    /// Duplicates the blobs, commitments, and proofs such that they exist
    /// `count` times.
    pub fn duplicate_blobs(mut self, count: usize) -> Self {
        self.request.blob_hashes = self
            .request
            .blob_hashes
            .into_iter()
            .cycle()
            .take(count)
            .collect();

        self.blobs = self.blobs.into_iter().cycle().take(count).collect();
        self.commitments = self.commitments.into_iter().cycle().take(count).collect();
        self.proofs = self.proofs.into_iter().cycle().take(count).collect();

        self
    }

    pub fn input(mut self, input: Bytes) -> Self {
        self.request.input = input;
        self
    }

    pub fn nonce(mut self, nonce: u64) -> Self {
        self.request.nonce = nonce;
        self
    }

    pub fn to(mut self, to: Address) -> Self {
        self.request.to = to;
        self
    }
}

impl Default for BlobTransactionBuilder {
    fn default() -> Self {
        let edr_chain_l1::PooledTransaction::Eip4844(pooled_transaction) =
            fake_pooled_transaction()
        else {
            unreachable!("Must be an EIP-4844 transaction")
        };

        let (transaction, blobs, commitments, proofs) = pooled_transaction.into_inner();
        let request = edr_chain_l1::request::Eip4844 {
            chain_id: transaction.chain_id,
            nonce: transaction.nonce,
            max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,
            max_fee_per_gas: transaction.max_fee_per_gas,
            gas_limit: transaction.gas_limit,
            to: transaction.to,
            value: transaction.value,
            input: transaction.input,
            access_list: transaction.access_list.into(),
            max_fee_per_blob_gas: transaction.max_fee_per_blob_gas,
            blob_hashes: transaction.blob_hashes,
        };

        Self {
            request,
            blobs,
            commitments,
            proofs,
        }
    }
}

pub fn fake_raw_transaction() -> Bytes {
    Bytes::from_str(include_str!("../fixtures/eip4844.txt"))
        .expect("failed to parse raw transaction")
}

pub fn fake_pooled_transaction() -> edr_chain_l1::PooledTransaction {
    let raw_transaction = fake_raw_transaction();

    edr_chain_l1::PooledTransaction::decode(&mut raw_transaction.as_ref())
        .expect("failed to decode raw transaction")
}

pub fn fake_transaction() -> edr_chain_l1::Signed {
    fake_pooled_transaction().into_payload()
}
