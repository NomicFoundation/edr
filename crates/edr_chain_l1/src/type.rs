use std::str::FromStr;

use edr_transaction::{IsEip4844, IsLegacy, RuintBaseConvertError, RuintParseError, U8};

use crate::signed;

/// The type of transaction.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Type {
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

impl From<Type> for u8 {
    fn from(t: Type) -> u8 {
        t as u8
    }
}

impl IsEip4844 for Type {
    fn is_eip4844(&self) -> bool {
        matches!(self, Type::Eip4844)
    }
}

impl IsLegacy for Type {
    fn is_legacy(&self) -> bool {
        matches!(self, Type::Legacy)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("{0}")]
    BaseConvertError(RuintBaseConvertError),
    #[error("Invalid digit: {0}")]
    InvalidDigit(char),
    #[error("Invalid radix. Only hexadecimal is supported.")]
    InvalidRadix,
    #[error("Unknown transaction type: {0}")]
    UnknownType(u8),
}

impl From<RuintParseError> for ParseError {
    fn from(error: RuintParseError) -> Self {
        match error {
            RuintParseError::InvalidDigit(c) => ParseError::InvalidDigit(c),
            RuintParseError::InvalidRadix(_) => ParseError::InvalidRadix,
            RuintParseError::BaseConvertError(error) => ParseError::BaseConvertError(error),
        }
    }
}

impl FromStr for Type {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_char_boundary(2) {
            let (prefix, rest) = s.split_at(2);
            if prefix == "0x" {
                let value = U8::from_str_radix(rest, 16)?;

                Type::try_from(value.to::<u8>()).map_err(ParseError::UnknownType)
            } else {
                Err(ParseError::InvalidRadix)
            }
        } else {
            Err(ParseError::InvalidRadix)
        }
    }
}

impl TryFrom<u8> for Type {
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

impl<'deserializer> serde::Deserialize<'deserializer> for Type {
    fn deserialize<D>(deserializer: D) -> Result<Type, D::Error>
    where
        D: serde::Deserializer<'deserializer>,
    {
        let value = U8::deserialize(deserializer)?;
        Type::try_from(value.to::<u8>()).map_err(serde::de::Error::custom)
    }
}

impl serde::Serialize for Type {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        U8::serialize(&U8::from(u8::from(*self)), serializer)
    }
}
