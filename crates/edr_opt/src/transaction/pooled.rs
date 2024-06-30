pub use edr_eth::transaction::pooled::{Eip155, Eip1559, Eip2930, Eip4844, Legacy};

use super::{Pooled, Signed};

/// An Optimism deposited pooled transaction.
pub type Deposited = super::signed::Deposited;

impl Pooled {
    /// Converts the pooled transaction into a signed transaction.
    pub fn into_payload(self) -> Signed {
        match self {
            Pooled::PreEip155Legacy(tx) => Signed::PreEip155Legacy(tx),
            Pooled::PostEip155Legacy(tx) => Signed::PostEip155Legacy(tx),
            Pooled::Eip2930(tx) => Signed::Eip2930(tx),
            Pooled::Eip1559(tx) => Signed::Eip1559(tx),
            Pooled::Eip4844(tx) => Signed::Eip4844(tx.into_payload()),
            Pooled::Deposited(tx) => Signed::Deposited(tx),
        }
    }
}
