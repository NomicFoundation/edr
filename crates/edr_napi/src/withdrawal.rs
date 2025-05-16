use edr_eth::Address;
use napi::bindgen_prelude::{BigInt, Uint8Array};
use napi_derive::napi;

use crate::cast::TryCast as _;

#[napi(object)]
pub struct Withdrawal {
    /// The index of withdrawal
    pub index: BigInt,
    /// The index of the validator that generated the withdrawal
    pub validator_index: BigInt,
    /// The recipient address for withdrawal value
    pub address: Uint8Array,
    /// The value contained in withdrawal
    pub amount: BigInt,
}

impl From<edr_eth::withdrawal::Withdrawal> for Withdrawal {
    fn from(withdrawal: edr_eth::withdrawal::Withdrawal) -> Self {
        Self {
            index: BigInt::from(withdrawal.index),
            validator_index: BigInt::from(withdrawal.validator_index),
            address: Uint8Array::with_data_copied(withdrawal.address),
            amount: BigInt {
                sign_bit: false,
                words: withdrawal.amount.as_limbs().to_vec(),
            },
        }
    }
}

impl TryFrom<Withdrawal> for edr_eth::withdrawal::Withdrawal {
    type Error = napi::Error;

    fn try_from(value: Withdrawal) -> Result<Self, Self::Error> {
        let index: u64 = BigInt::try_cast(value.index)?;
        let validator_index: u64 = BigInt::try_cast(value.validator_index)?;
        let amount = BigInt::try_cast(value.amount)?;
        let address = Address::from_slice(&value.address);

        Ok(Self {
            index,
            validator_index,
            address,
            amount,
        })
    }
}
