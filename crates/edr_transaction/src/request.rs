mod eip155;
mod eip1559;
mod eip2930;
mod eip4844;
mod eip7702;
mod legacy;

use edr_primitives::Address;

pub use self::{
    eip155::Eip155, eip1559::Eip1559, eip2930::Eip2930, eip4844::Eip4844, eip7702::Eip7702,
    legacy::Legacy,
};

/// A transaction request and the sender's address.
#[derive(Clone, Debug)]
pub struct TransactionRequestAndSender<RequestT> {
    /// The transaction request.
    pub request: RequestT,
    /// The sender's address.
    pub sender: Address,
}
