//! Types and errors for EIP-712 canonical type parsing.

use alloy_dyn_abi::eip712_parser::EncodeType;

use crate::Eip712Type;

/// Opens the member list of a type in an `encodeType` string.
const MEMBER_LIST_START: char = '(';

/// Why an input that parses but declares no type is rejected.
const NO_TYPE_DEFINITION: &str = "no parseable type definition found";

/// Errors that can occur while parsing or canonicalizing an EIP-712 type
/// definition.
#[derive(Debug, thiserror::Error)]
pub enum Eip712Error {
    /// The input could not be parsed as an EIP-712 `encodeType` string.
    #[error("failed to parse EIP-712 canonical type {input}: {reason}")]
    Parse {
        /// The offending input.
        input: String,
        /// Why parsing failed.
        reason: String,
    },

    /// The parsed type could not be canonicalized (e.g. a referenced struct
    /// type was not inlined).
    #[error("failed to canonicalize EIP-712 type {input}: {reason}")]
    Canonicalize {
        /// The offending input.
        input: String,
        /// Why canonicalization failed.
        reason: String,
    },
}

impl Eip712Type {
    /// Parses and canonicalizes an EIP-712 type definition, extracting its
    /// primary-type name.
    pub fn parse(input: &str) -> std::result::Result<Self, Eip712Error> {
        let encode_type = EncodeType::parse(input).map_err(|error| Eip712Error::Parse {
            input: input.to_string(),
            reason: error.to_string(),
        })?;

        if encode_type.types.is_empty() {
            return Err(Eip712Error::Parse {
                input: input.to_string(),
                reason: NO_TYPE_DEFINITION.into(),
            });
        }

        let canonical_definition =
            encode_type
                .canonicalize()
                .map_err(|error| Eip712Error::Canonicalize {
                    input: input.to_string(),
                    reason: error.to_string(),
                })?;

        // The canonical form starts with the primary type whatever order the
        // input listed its types in.
        let (name, _) = canonical_definition
            .split_once(MEMBER_LIST_START)
            .expect("a canonicalized type definition opens its primary type's member list");

        Ok(Self {
            name: name.to_owned(),
            canonical_definition,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_MAIL_CANONICAL: &str = "Mail(address from,address to,string contents)";

    #[test]
    fn parses_canonical_definition() {
        let def = Eip712Type::parse(SIMPLE_MAIL_CANONICAL).unwrap();
        assert_eq!(def.name(), "Mail");
        assert_eq!(def.canonical_definition(), SIMPLE_MAIL_CANONICAL);
    }

    #[test]
    fn normalizes_whitespace() {
        // Extra whitespace after commas — not canonical per EIP-712.
        let noisy = "Mail(address from, address to, string contents)";
        let def = Eip712Type::parse(noisy).unwrap();
        assert_eq!(def.canonical_definition(), SIMPLE_MAIL_CANONICAL);
    }

    #[test]
    fn sorts_referenced_types_alphabetically() {
        // EIP-712: "the set of referenced struct types is collected,
        // sorted by name and appended to the encoding". Input here has
        // Person before Asset, so canonical output must swap them.
        let non_canonical = "Transaction(Person from,Person to,Asset tx)\
                             Person(address wallet,string name)\
                             Asset(address token,uint256 amount)";
        let expected = "Transaction(Person from,Person to,Asset tx)\
                        Asset(address token,uint256 amount)\
                        Person(address wallet,string name)";

        let def = Eip712Type::parse(non_canonical).unwrap();
        assert_eq!(def.name(), "Transaction");
        assert_eq!(def.canonical_definition(), expected);
    }

    #[test]
    fn names_the_primary_type_of_a_reordered_input() {
        // The referenced type comes first in the input, but EIP-712 puts the
        // primary type first in the canonical form.
        let def = Eip712Type::parse("Person(address wallet)Mail(Person from)").unwrap();

        assert_eq!(def.name(), "Mail");
        assert!(
            def.canonical_definition().starts_with("Mail("),
            "{}",
            def.canonical_definition()
        );
    }

    #[test]
    fn rejects_empty_input() {
        let err = Eip712Type::parse("").unwrap_err();
        assert!(matches!(
            &err,
            Eip712Error::Parse { reason, .. } if reason == NO_TYPE_DEFINITION
        ));
    }

    #[test]
    fn rejects_unparseable_input() {
        let err = Eip712Type::parse("not a type definition").unwrap_err();
        assert!(matches!(&err, Eip712Error::Parse { .. }));
    }

    #[test]
    fn rejects_unresolved_nested_types() {
        // Mail references Person but does not inline its definition.
        // Canonicalization must resolve every referenced type.
        let err = Eip712Type::parse("Mail(Person from,Person to,string contents)").unwrap_err();
        assert!(matches!(&err, Eip712Error::Canonicalize { .. }));
    }
}
