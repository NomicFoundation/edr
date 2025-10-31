use std::str::FromStr;

use edr_primitives::U8;
use edr_transaction::{IsEip4844, ParseError};

use super::{signed, OpTransactionType};

impl From<OpTransactionType> for u8 {
    fn from(t: OpTransactionType) -> u8 {
        t as u8
    }
}

impl FromStr for OpTransactionType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_char_boundary(2) {
            let (prefix, rest) = s.split_at(2);
            if prefix == "0x" {
                let value = U8::from_str_radix(rest, 16)?;

                OpTransactionType::try_from(value.to::<u8>()).map_err(ParseError::UnknownType)
            } else {
                Err(ParseError::InvalidRadix)
            }
        } else {
            Err(ParseError::InvalidRadix)
        }
    }
}

impl IsEip4844 for OpTransactionType {
    fn is_eip4844(&self) -> bool {
        *self == OpTransactionType::Eip4844
    }
}

impl TryFrom<u8> for OpTransactionType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            signed::Legacy::TYPE => Ok(Self::Legacy),
            signed::Eip2930::TYPE => Ok(Self::Eip2930),
            signed::Eip1559::TYPE => Ok(Self::Eip1559),
            signed::Eip4844::TYPE => Ok(Self::Eip4844),
            signed::Eip7702::TYPE => Ok(Self::Eip7702),
            signed::Deposit::TYPE => Ok(Self::Deposit),
            value => Err(value),
        }
    }
}

impl<'deserializer> serde::Deserialize<'deserializer> for OpTransactionType {
    fn deserialize<D>(deserializer: D) -> Result<OpTransactionType, D::Error>
    where
        D: serde::Deserializer<'deserializer>,
    {
        let value = U8::deserialize(deserializer)?;
        OpTransactionType::try_from(value.to::<u8>()).map_err(serde::de::Error::custom)
    }
}

impl serde::Serialize for OpTransactionType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        U8::serialize(&U8::from(u8::from(*self)), serializer)
    }
}

#[cfg(test)]
mod tests {
    use crate::transaction::OpTransactionType;

    fn assert_conversion(expected_conversion: OpTransactionType) {
        let value: u8 = expected_conversion.into();
        assert_eq!(OpTransactionType::try_from(value), Ok(expected_conversion));
    }

    #[test]
    fn test_transaction_type_conversion() {
        let possible_values = [
            OpTransactionType::Deposit,
            OpTransactionType::Eip1559,
            OpTransactionType::Eip2930,
            OpTransactionType::Eip4844,
            OpTransactionType::Eip7702,
            OpTransactionType::Legacy,
        ];
        for transaction_type in possible_values {
            // using match to ensure we are covering all variants
            match transaction_type {
                OpTransactionType::Eip1559 => assert_conversion(OpTransactionType::Eip1559),
                OpTransactionType::Eip2930 => assert_conversion(OpTransactionType::Eip2930),
                OpTransactionType::Eip4844 => assert_conversion(OpTransactionType::Eip4844),
                OpTransactionType::Eip7702 => assert_conversion(OpTransactionType::Eip7702),
                OpTransactionType::Deposit => assert_conversion(OpTransactionType::Deposit),
                OpTransactionType::Legacy => assert_conversion(OpTransactionType::Legacy),
            }
        }
    }
}
