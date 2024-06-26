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

#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
    Legacy = 0,
    Eip2930 = 1,
    Eip1559 = 2,
    Eip4844 = 3,
    Deposited = 0x7E,
}

impl TryFrom<u64> for Type {
    type Error = u64;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Legacy),
            1 => Ok(Self::Eip2930),
            2 => Ok(Self::Eip1559),
            3 => Ok(Self::Eip4844),
            0x7E => Ok(Self::Deposited),
            _ => Err(value),
        }
    }
}
