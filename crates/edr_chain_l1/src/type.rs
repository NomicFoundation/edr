use std::str::FromStr;

use edr_primitives::U8;
use edr_transaction::{IsEip4844, IsLegacy, ParseError};

use crate::signed;

/// The type of transaction.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum L1TransactionType {
    /// Legacy transaction
    Legacy = signed::Legacy::TYPE,
    /// EIP-2930 transaction
    Eip2930 = signed::Eip2930::TYPE,
    /// EIP-1559 transaction
    Eip1559 = signed::Eip1559::TYPE,
    /// EIP-4844 transaction
    Eip4844 = signed::Eip4844::TYPE,
    /// EIP-7702 transaction
    Eip7702 = signed::Eip7702::TYPE,
}

impl From<L1TransactionType> for u8 {
    fn from(t: L1TransactionType) -> u8 {
        t as u8
    }
}

impl IsEip4844 for L1TransactionType {
    fn is_eip4844(&self) -> bool {
        matches!(self, L1TransactionType::Eip4844)
    }
}

impl IsLegacy for L1TransactionType {
    fn is_legacy(&self) -> bool {
        matches!(self, L1TransactionType::Legacy)
    }
}

impl FromStr for L1TransactionType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_char_boundary(2) {
            let (prefix, rest) = s.split_at(2);
            if prefix == "0x" {
                let value = U8::from_str_radix(rest, 16)?;

                L1TransactionType::try_from(value.to::<u8>()).map_err(ParseError::UnknownType)
            } else {
                Err(ParseError::InvalidRadix)
            }
        } else {
            Err(ParseError::InvalidRadix)
        }
    }
}

impl TryFrom<u8> for L1TransactionType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            signed::Legacy::TYPE => Ok(Self::Legacy),
            signed::Eip2930::TYPE => Ok(Self::Eip2930),
            signed::Eip1559::TYPE => Ok(Self::Eip1559),
            signed::Eip4844::TYPE => Ok(Self::Eip4844),
            signed::Eip7702::TYPE => Ok(Self::Eip7702),
            value => Err(value),
        }
    }
}

impl<'deserializer> serde::Deserialize<'deserializer> for L1TransactionType {
    fn deserialize<D>(deserializer: D) -> Result<L1TransactionType, D::Error>
    where
        D: serde::Deserializer<'deserializer>,
    {
        let value = U8::deserialize(deserializer)?;
        L1TransactionType::try_from(value.to::<u8>()).map_err(serde::de::Error::custom)
    }
}

impl serde::Serialize for L1TransactionType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        U8::serialize(&U8::from(u8::from(*self)), serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_conversion(expected_conversion: L1TransactionType) {
        let value: u8 = expected_conversion.into();
        assert_eq!(L1TransactionType::try_from(value), Ok(expected_conversion));
    }

    #[test]
    fn test_transaction_type_conversion() {
        let possible_values = [
            L1TransactionType::Eip1559,
            L1TransactionType::Eip2930,
            L1TransactionType::Eip4844,
            L1TransactionType::Eip7702,
            L1TransactionType::Legacy,
        ];
        for transaction_type in possible_values {
            // using match to ensure we are covering all variants
            match transaction_type {
                L1TransactionType::Eip1559 => assert_conversion(L1TransactionType::Eip1559),
                L1TransactionType::Eip2930 => assert_conversion(L1TransactionType::Eip2930),
                L1TransactionType::Eip4844 => assert_conversion(L1TransactionType::Eip4844),
                L1TransactionType::Eip7702 => assert_conversion(L1TransactionType::Eip7702),
                L1TransactionType::Legacy => assert_conversion(L1TransactionType::Legacy),
            }
        }
    }
}
