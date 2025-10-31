//! Ethereum L1 RPC types

pub mod block;
pub mod call;
pub mod receipt;
pub mod transaction;

pub type Block<TransactionT> = self::block::L1RpcBlock<TransactionT>;

pub type BlockReceipt = self::receipt::L1RpcTransactionReceipt;

pub type TransactionRequest = self::transaction::L1RpcTransactionRequest;
