use super::request::Eip7702;

/// The type of transaction.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransactionType {
    Legacy = 0,
    Eip2930 = 1,
    Eip1559 = 2,
    Eip4844 = 3,
    Eip7702 = Eip7702::TYPE as u64,
}

impl From<TransactionType> for u64 {
    fn from(t: TransactionType) -> u64 {
        t as u64
    }
}
