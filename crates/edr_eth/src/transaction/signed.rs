mod eip155;
mod eip1559;
mod eip2930;
mod eip4844;
mod eip7702;
mod legacy;

use k256::SecretKey;

pub use self::{
    eip155::Eip155,
    eip1559::Eip1559,
    eip2930::Eip2930,
    eip4844::Eip4844,
    eip7702::Eip7702,
    legacy::{Legacy, PreOrPostEip155},
};
use crate::{signature::SignatureError, Address};

/// Trait for signing a transaction request with a fake signature.
pub trait FakeSign {
    /// The type of the signed transaction.
    type Signed;

    /// Signs the transaction with a fake signature.
    fn fake_sign(self, sender: Address) -> Self::Signed;
}

pub trait Sign {
    /// The type of the signed transaction.
    type Signed;

    /// Signs the transaction with the provided secret key, belonging to the
    /// provided sender's address.
    ///
    /// # Safety
    ///
    /// The `caller` and `secret_key` must correspond to the same account.
    unsafe fn sign_for_sender_unchecked(
        self,
        secret_key: &SecretKey,
        caller: Address,
    ) -> Result<Self::Signed, SignatureError>;
}
