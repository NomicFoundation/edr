mod r#type;

/// Types for transaction gossip (aka pooled transactions).
pub mod pooled;
/// Types for transaction requests.
pub mod request;
/// Types for signed transactions.
pub mod signed;

pub use op_revm::{
    transaction::OpTxTr as OpTxTrait, OpTransaction as OpTxEnv,
    OpTransactionError as InvalidTransaction,
};

/// An OP signed transaction, used in blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpSignedTransaction {
    /// Legacy transaction before EIP-155
    PreEip155Legacy(signed::Legacy),
    /// Legacy transaction after EIP-155
    PostEip155Legacy(signed::Eip155),
    /// EIP-2930 transaction
    Eip2930(signed::Eip2930),
    /// EIP-1559 transaction
    Eip1559(signed::Eip1559),
    /// EIP-4844 transaction
    Eip4844(signed::Eip4844),
    /// EIP-7702 transaction
    Eip7702(signed::Eip7702),
    /// OP deposit transaction
    Deposit(signed::Deposit),
}

/// The type of OP transaction.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OpTransactionType {
    /// OP legacy transaction
    Legacy = signed::Legacy::TYPE,
    /// OP EIP-2930 transaction
    Eip2930 = signed::Eip2930::TYPE,
    /// OP EIP-1559 transaction
    Eip1559 = signed::Eip1559::TYPE,
    /// OP EIP-4844 transaction
    Eip4844 = signed::Eip4844::TYPE,
    /// OP EIP-7702 transaction
    Eip7702 = signed::Eip7702::TYPE,
    /// OP deposit transaction
    Deposit = signed::Deposit::TYPE,
}
