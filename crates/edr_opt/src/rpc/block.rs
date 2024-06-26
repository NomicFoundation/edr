use edr_eth::block::{BlobGas, Header};
use edr_evm::{IntoRemoteBlock, RemoteBlock, RemoteBlockCreationError};
use edr_rpc_eth::client::EthRpcClient;

use crate::{rpc, transaction, OptimismChainSpec};

impl IntoRemoteBlock<OptimismChainSpec> for edr_rpc_eth::Block<rpc::Transaction> {
    fn into_remote_block(
        self,
        rpc_client: std::sync::Arc<EthRpcClient<OptimismChainSpec>>,
        runtime: tokio::runtime::Handle,
    ) -> Result<RemoteBlock<OptimismChainSpec>, RemoteBlockCreationError> {
        let header = Header {
            parent_hash: self.parent_hash,
            ommers_hash: self.sha3_uncles,
            beneficiary: self.miner.ok_or(RemoteBlockCreationError::MissingMiner)?,
            state_root: self.state_root,
            transactions_root: self.transactions_root,
            receipts_root: self.receipts_root,
            logs_bloom: self.logs_bloom,
            difficulty: self.difficulty,
            number: self.number.ok_or(RemoteBlockCreationError::MissingNumber)?,
            gas_limit: self.gas_limit,
            gas_used: self.gas_used,
            timestamp: self.timestamp,
            extra_data: self.extra_data,
            // TODO don't accept remote blocks with missing mix hash,
            // see https://github.com/NomicFoundation/edr/issues/518
            mix_hash: self.mix_hash.unwrap_or_default(),
            nonce: self.nonce.ok_or(RemoteBlockCreationError::MissingNonce)?,
            base_fee_per_gas: self.base_fee_per_gas,
            withdrawals_root: self.withdrawals_root,
            blob_gas: self.blob_gas_used.and_then(|gas_used| {
                self.excess_blob_gas.map(|excess_gas| BlobGas {
                    gas_used,
                    excess_gas,
                })
            }),
            parent_beacon_block_root: self.parent_beacon_block_root,
        };

        let transactions = self
            .transactions
            .into_iter()
            .map(transaction::Signed::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        let hash = self.hash.ok_or(RemoteBlockCreationError::MissingHash)?;

        Ok(RemoteBlock {
            header,
            transactions,
            receipts: OnceLock::new(),
            ommer_hashes: self.uncles,
            withdrawals: self.withdrawals,
            hash,
            size: self.size,
            rpc_client,
            runtime,
        })
    }
}
