use revm_primitives::keccak256;

use crate::{transaction::sealed::ComputeTransactionHash, utils::envelop_bytes, B256};

pub type Eip7702 = alloy_consensus::transaction::TxEip7702;

impl ComputeTransactionHash for Eip7702 {
    fn compute_transaction_hash(&self) -> B256 {
        let encoded = alloy_rlp::encode(self);
        let enveloped = envelop_bytes(Eip7702::tx_type().into(), &encoded);

        keccak256(enveloped)
    }
}
