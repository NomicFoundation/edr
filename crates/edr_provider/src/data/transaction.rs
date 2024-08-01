use std::sync::Arc;

use edr_eth::{
    chain_spec::L1ChainSpec,
    transaction::{IsEip4844 as _, SignedTransaction as _, Transaction as _, TransactionType as _},
    SpecId, U256,
};
use edr_evm::{blockchain::BlockchainError, chain_spec::ChainSpec, transaction, SyncBlock};
use edr_rpc_eth::RpcTypeFrom;

use super::TransactionAndBlock;

impl RpcTypeFrom<TransactionAndBlock<L1ChainSpec>> for edr_rpc_eth::Transaction {
    type Hardfork = SpecId;

    fn rpc_type_from(value: &TransactionAndBlock<L1ChainSpec>, hardfork: Self::Hardfork) -> Self {
        let TransactionAndBlock {
            transaction,
            block_data,
            is_pending,
        } = value;
        let block = block_data.as_ref().map(|b| &b.block);
        let header = block.map(|b| b.header());

        let gas_price = match &transaction {
            transaction::Signed::PreEip155Legacy(tx) => tx.gas_price,
            transaction::Signed::PostEip155Legacy(tx) => tx.gas_price,
            transaction::Signed::Eip2930(tx) => tx.gas_price,
            transaction::Signed::Eip1559(_) | transaction::Signed::Eip4844(_) => {
                gas_price_for_post_eip1559(&transaction, block)
            }
        };

        let chain_id = match &transaction {
            // Following Hardhat in not returning `chain_id` for `PostEip155Legacy` legacy transactions
            // even though the chain id would be recoverable.
            transaction::Signed::PreEip155Legacy(_) | transaction::Signed::PostEip155Legacy(_) => {
                None
            }
            transaction::Signed::Eip2930(tx) => Some(tx.chain_id),
            transaction::Signed::Eip1559(tx) => Some(tx.chain_id),
            transaction::Signed::Eip4844(tx) => Some(tx.chain_id),
        };

        let show_transaction_type = hardfork >= SpecId::BERLIN;
        let is_typed_transaction = transaction.transaction_type() > transaction::Type::Legacy;
        let transaction_type = if show_transaction_type || is_typed_transaction {
            Some(transaction.transaction_type())
        } else {
            None
        };

        let signature = transaction.signature();
        let (block_hash, block_number) = if *is_pending {
            (None, None)
        } else {
            header
                .map(|header| (header.hash(), U256::from(header.number)))
                .unzip()
        };

        let transaction_index = if *is_pending {
            None
        } else {
            block_data.as_ref().map(|bd| bd.transaction_index)
        };

        let access_list = if transaction.transaction_type() >= transaction::Type::Eip2930 {
            Some(transaction.access_list().to_vec())
        } else {
            None
        };

        let blob_versioned_hashes = if transaction.transaction_type().is_eip4844() {
            Some(transaction.blob_hashes().to_vec())
        } else {
            None
        };

        Self {
            hash: *transaction.transaction_hash(),
            nonce: transaction.nonce(),
            block_hash,
            block_number,
            transaction_index,
            from: *transaction.caller(),
            to: transaction.kind().to().copied(),
            value: *transaction.value(),
            gas_price,
            gas: U256::from(transaction.gas_limit()),
            input: transaction.data().clone(),
            v: signature.v(),
            // Following Hardhat in always returning `v` instead of `y_parity`.
            y_parity: None,
            r: signature.r(),
            s: signature.s(),
            chain_id,
            transaction_type: transaction_type.map(u8::from),
            access_list,
            max_fee_per_gas: transaction.max_fee_per_gas(),
            max_priority_fee_per_gas: transaction.max_priority_fee_per_gas().cloned(),
            max_fee_per_blob_gas: transaction.max_fee_per_blob_gas().cloned(),
            blob_versioned_hashes,
        }
    }
}

fn gas_price_for_post_eip1559<ChainSpecT: ChainSpec>(
    signed_transaction: &transaction::Signed,
    block: Option<&Arc<dyn SyncBlock<ChainSpecT, Error = BlockchainError<ChainSpecT>>>>,
) -> U256 {
    let max_fee_per_gas = signed_transaction
        .max_fee_per_gas()
        .expect("Transaction must be post EIP-1559 transaction.");
    let max_priority_fee_per_gas = *signed_transaction
        .max_priority_fee_per_gas()
        .expect("Transaction must be post EIP-1559 transaction.");

    if let Some(block) = block {
        let base_fee_per_gas = block.header().base_fee_per_gas.expect(
            "Transaction must have base fee per gas in block metadata if EIP-1559 is active.",
        );
        let priority_fee_per_gas = max_priority_fee_per_gas.min(max_fee_per_gas - base_fee_per_gas);
        base_fee_per_gas + priority_fee_per_gas
    } else {
        // We are following Hardhat's behavior of returning the max fee per gas for
        // pending transactions.
        max_fee_per_gas
    }
}
