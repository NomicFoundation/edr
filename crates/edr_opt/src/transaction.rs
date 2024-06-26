/// Pooled transaction types
pub mod pooled;
/// Signed transaction types
pub mod signed;

/// An Optimism pooled transaction, used to communicate between node pools.
pub enum Pooled {
    /// Legacy transaction before EIP-155
    PreEip155Legacy(pooled::Legacy),
    /// Legacy transaction after EIP-155
    PostEip155Legacy(pooled::Eip155),
    /// EIP-2930 transaction
    Eip2930(pooled::Eip2930),
    /// EIP-1559 transaction
    Eip1559(pooled::Eip1559),
    /// EIP-4844 transaction
    Eip4844(pooled::Eip4844),
    /// Optimism deposited transaction
    Deposited(pooled::Deposited),
}

/// An optimism signed transaction, used in blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signed {
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
    /// Optimism deposited transaction
    Deposited(signed::Deposited),
}

/// The type of Optimism transaction.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
    /// Optimism legacy transaction
    Legacy = signed::Legacy::TYPE,
    /// Optimism EIP-2930 transaction
    Eip2930 = signed::Eip2930::TYPE,
    /// Optimism EIP-1559 transaction
    Eip1559 = signed::Eip1559::TYPE,
    /// Optimism EIP-4844 transaction
    Eip4844 = signed::Eip4844::TYPE,
    /// Optimism deposited transaction
    Deposited = signed::Deposited::TYPE,
}

impl TryFrom<u8> for Type {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            signed::Legacy::TYPE => Ok(Self::Legacy),
            signed::Eip2930::TYPE => Ok(Self::Eip2930),
            signed::Eip1559::TYPE => Ok(Self::Eip1559),
            signed::Eip4844::TYPE => Ok(Self::Eip4844),
            signed::Deposited::TYPE => Ok(Self::Deposited),
            value => Err(value),
        }
    }
}
