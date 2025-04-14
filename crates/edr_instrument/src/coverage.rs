use anyhow::Result;
use edr_eth::B256;
use semver::Version;
use serde::Serialize;
use sha3::{Digest, Keccak256};
use slang_solidity::{
    cst::{Node, NonterminalKind},
    parser::Parser as SolidityParser,
};

#[derive(Serialize)]
pub struct InstrumentationMetadata {
    pub tag: B256,
    pub r#type: &'static str,
    pub start_utf16: usize,
    pub end_utf16: usize,
}

pub struct InstrumentationResult {
    pub source: String,
    pub metadata: Vec<InstrumentationMetadata>,
}

/// Computes a deterministic hash for a statement in a Solidity file.
///
/// For the time being, we're assuming that compilation configuration has no
/// impact on the statement hash. This is a simplification, but is considered a
/// reasonable trade-off for the current use case.
fn compute_deterministic_hash_for_statement(
    source_id: &str,
    content_hash: &B256,
    statement_counter: u64,
) -> B256 {
    let mut hasher = Keccak256::new();

    hasher.update(source_id);
    hasher.update(content_hash);
    hasher.update(statement_counter.to_le_bytes());

    let hash = hasher.finalize();
    B256::new(hash.into())
}

pub fn instrument_internal(
    content: &str,
    solidity_version: &str,
    source_id: &str,
) -> Result<InstrumentationResult> {
    let version = Version::parse(solidity_version)
        .map_err(|e| anyhow::anyhow!("Invalid Solidity version: {e}"))?;
    let parser = SolidityParser::create(version)?;
    let parse_result = parser.parse(SolidityParser::ROOT_KIND, content);
    if !parse_result.is_valid() {
        return Err(anyhow::anyhow!("Failed to parse Solidity file"));
    }

    // Compute the content hash once
    let content_hash = B256::new(Keccak256::digest(content).into());

    let mut instrumented_source = String::new();
    let mut metadata = Vec::new();

    let mut statement_counter: u64 = 0;
    let mut cursor = parse_result.create_tree_cursor();
    while cursor.go_to_next() {
        match cursor.node() {
            Node::Nonterminal(node) if node.kind == NonterminalKind::Statement => {
                statement_counter += 1;

                let hash = compute_deterministic_hash_for_statement(
                    source_id,
                    &content_hash,
                    statement_counter,
                );

                instrumented_source.push_str(&format!("__HardhatCoverage.sendHit({hash}); "));

                let range = cursor.text_range();
                metadata.push(InstrumentationMetadata {
                    tag: hash,
                    r#type: "statement",
                    start_utf16: range.start.utf16,
                    end_utf16: range.end.utf16,
                });
            }
            Node::Terminal(node) => {
                instrumented_source.push_str(&node.text);
            }
            Node::Nonterminal(_) => {}
        }
    }

    instrumented_source.push_str("\nimport \"hardhat/coverage.sol\";");

    Ok(InstrumentationResult {
        source: instrumented_source,
        metadata,
    })

    // serde_json::to_string_pretty(&metadata)
    //.context("Failed to serialize metadata")?
}

#[cfg(test)]
mod tests {
    use edr_eth::b256;

    use super::*;

    const FIXTURE: &str = "\
contract Test {\
    function test() public {\
        uint x = 1;\
        uint y = 2;\
        uint z = x + y;\
    }\
}";

    fn assert_metadata(
        metadata: &InstrumentationMetadata,
        expected_tag: B256,
        expected_text: &str,
    ) {
        assert_eq!(metadata.tag, expected_tag);
        assert_eq!(metadata.r#type, "statement");

        let text = select_text(FIXTURE, metadata.start_utf16, metadata.end_utf16);
        assert_eq!(text, expected_text);
    }

    fn select_text(content: &str, start_utf16: usize, end_utf16: usize) -> String {
        let start = content
            .char_indices()
            .nth(start_utf16)
            .map_or(0, |(i, _)| i);
        let end = content
            .char_indices()
            .nth(end_utf16)
            .map_or(content.len(), |(i, _)| i);

        content[start..end].to_string()
    }

    #[test]
    fn determinism() {
        let result =
            instrument_internal(FIXTURE, "0.8.0", "test.sol").expect("Failed to instrument");

        let tags = result
            .metadata
            .iter()
            .map(|InstrumentationMetadata { tag, .. }| tag)
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(
            tags,
            vec![
                b256!("0x637146053532ee0af7483c6b648017252b64f8d3f301f67fb9830f26ff3a0cf6"),
                b256!("0x9960a8ff2acba9f756cd0057a1aade2a4a9c37ce99b7b440f00c3fdc9baacbb3"),
                b256!("0xec27ebd0f137626f1cc71e675061e2d04c26e38a572e8f9738c0d28e009d058c"),
            ]
        );
    }

    #[test]
    fn import() {
        let result =
            instrument_internal(FIXTURE, "0.8.0", "test.sol").expect("Failed to instrument");

        assert!(result.source.contains("import \"hardhat/coverage.sol\";"));
    }

    #[test]
    fn instrumentation() {
        let result =
            instrument_internal(FIXTURE, "0.8.0", "test.sol").expect("Failed to instrument");

        assert!(result.source.contains("__HardhatCoverage.sendHit("));
        assert!(result.source.contains(");"));
    }

    #[test]
    fn mapping() {
        let result =
            instrument_internal(FIXTURE, "0.8.0", "test.sol").expect("Failed to instrument");

        assert_eq!(result.metadata.len(), 3);

        assert_metadata(
            &result.metadata[0],
            b256!("0x637146053532ee0af7483c6b648017252b64f8d3f301f67fb9830f26ff3a0cf6"),
            "uint x = 1;",
        );

        assert_metadata(
            &result.metadata[1],
            b256!("0x9960a8ff2acba9f756cd0057a1aade2a4a9c37ce99b7b440f00c3fdc9baacbb3"),
            "uint y = 2;",
        );

        assert_metadata(
            &result.metadata[2],
            b256!("0xec27ebd0f137626f1cc71e675061e2d04c26e38a572e8f9738c0d28e009d058c"),
            "uint z = x + y;",
        );
    }
}
