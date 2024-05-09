/// The type of transaction.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransactionType {
    Legacy = 0,
    Eip2930 = 1,
    Eip1559 = 2,
    Eip4844 = 3,
}

impl From<TransactionType> for u64 {
    fn from(t: TransactionType) -> u64 {
        t as u64
    }
}
