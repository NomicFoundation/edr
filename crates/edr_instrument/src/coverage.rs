use edr_eth::B256;
pub use semver::Version;
use serde::Serialize;
use sha3::{Digest, Keccak256};
use slang_solidity::{
    cst::{Edge, EdgeLabel, Node, NodeKind, NonterminalKind, TerminalKind},
    parser::{ParseError, Parser as SolidityParser, ParserInitializationError},
};

#[derive(Debug, Serialize)]
pub struct InstrumentationMetadata {
    pub tag: B256,
    pub kind: &'static str,
    pub start_utf16: usize,
    pub end_utf16: usize,
}

#[derive(Debug)]
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

#[derive(Debug, thiserror::Error)]
pub enum InstrumentationError {
    #[error(transparent)]
    Initialization(#[from] ParserInitializationError),
    #[error("Invalid library path.")]
    InvalidLibraryPath {
        errors: Vec<ParseError>,
        import: String,
    },
    #[error("Failed to parse Solidity file.")]
    InvalidSourceCode { errors: Vec<ParseError> },
}

pub fn instrument_code(
    source_code: &str,
    source_id: &str,
    solidity_version: Version,
    coverage_library_path: &str,
) -> Result<InstrumentationResult, InstrumentationError> {
    let parser = SolidityParser::create(solidity_version)?;
    let parsed_file = parser.parse_file_contents(source_code);
    if !parsed_file.is_valid() {
        return Err(InstrumentationError::InvalidSourceCode {
            errors: parsed_file.errors().clone(),
        });
    }

    let coverage_library_import = format!("\nimport \"{coverage_library_path}\";");
    let parsed_library_import =
        parser.parse_nonterminal(NonterminalKind::ImportDirective, &coverage_library_import);

    if !parsed_library_import.is_valid() {
        return Err(InstrumentationError::InvalidLibraryPath {
            errors: parsed_library_import.errors().clone(),
            import: coverage_library_import,
        });
    }

    // Compute the content hash once
    let content_hash = B256::new(Keccak256::digest(source_code).into());

    let mut instrumented_source = String::new();
    let mut metadata = Vec::new();

    let mut statement_counter: u64 = 0;

    // Calculate metadata for statements using the correct text ranges
    let mut cursor = parsed_file.create_tree_cursor();
    while !cursor.is_completed() {
        if let NodeKind::Nonterminal(NonterminalKind::Statement) = cursor.node().kind() {
            statement_counter += 1;

            let hash = compute_deterministic_hash_for_statement(
                source_id,
                &content_hash,
                statement_counter,
            );

            let text_range = cursor.text_range();
            metadata.push(InstrumentationMetadata {
                tag: hash,
                kind: "statement",
                start_utf16: text_range.start.utf16,
                end_utf16: text_range.end.utf16,
            });
        }

        cursor.go_to_next();
    }

    let mut metadata_iter = metadata.iter();
    let mut queue = vec![parsed_file.create_tree_cursor()];
    'queue: while let Some(mut cursor) = queue.pop() {
        while !cursor.is_completed() {
            match cursor.node() {
                Node::Nonterminal(node) => {
                    if node.kind == NonterminalKind::Statement {
                        let parent = cursor
                            .ancestors()
                            .next()
                            .expect("Cursor should have a parent");

                        if matches!(
                            parent.kind,
                            NonterminalKind::IfStatement
                                | NonterminalKind::ElseBranch
                                | NonterminalKind::ForStatement
                                | NonterminalKind::WhileStatement
                                | NonterminalKind::DoWhileStatement
                        ) {
                            let block = Node::nonterminal(
                                NonterminalKind::Block,
                                vec![
                                    Edge {
                                        label: EdgeLabel::OpenBrace,
                                        node: Node::terminal(
                                            TerminalKind::OpenBrace,
                                            "{".to_owned(),
                                        ),
                                    },
                                    Edge {
                                        label: EdgeLabel::Statements,
                                        node: Node::nonterminal(
                                            NonterminalKind::Statements,
                                            vec![Edge {
                                                label: EdgeLabel::Item,
                                                node: Node::Nonterminal(node),
                                            }],
                                        ),
                                    },
                                    Edge {
                                        label: EdgeLabel::CloseBrace,
                                        node: Node::terminal(
                                            TerminalKind::CloseBrace,
                                            "}".to_owned(),
                                        ),
                                    },
                                ],
                            );

                            // The text offset doesn't matter as we already calculated text offsets
                            // for the markers in an earlier step
                            let new_cursor = block.create_cursor(cursor.text_offset());

                            // Skip all descendants as `new_cursor` will already iterate those
                            cursor.go_to_next_non_descendant();

                            queue.push(cursor);
                            queue.push(new_cursor);

                            continue 'queue;
                        } else {
                            let metadata = metadata_iter
                                .next()
                                .expect("Metadata should exist for statement");

                            let instrumentation_statement =
                                format!("__HardhatCoverage.sendHit({hash}); ", hash = metadata.tag);

                            instrumented_source.push_str(&instrumentation_statement);
                        }
                    }
                }
                Node::Terminal(node) => {
                    instrumented_source.push_str(&node.text);
                }
            }

            cursor.go_to_next();
        }
    }

    instrumented_source.push_str(&coverage_library_import);

    Ok(InstrumentationResult {
        source: instrumented_source,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use edr_eth::b256;

    use super::*;

    const FIXTURE: &str = include_str!("../../../data/contracts/instrumentation.sol");
    const LIBRARY_PATH: &str = "__hardhat_coverage.sol";

    fn assert_metadata(
        metadata: &InstrumentationMetadata,
        expected_tag: B256,
        expected_text: &str,
    ) {
        assert_eq!(metadata.tag, expected_tag);
        assert_eq!(metadata.kind, "statement");

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
        let version = Version::parse("0.8.0").expect("Failed to parse version");
        let result = instrument_code(FIXTURE, "instrumentation.sol", version, LIBRARY_PATH)
            .expect("Failed to instrument");

        let tags = result
            .metadata
            .iter()
            .map(|InstrumentationMetadata { tag, .. }| tag)
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(
            tags,
            vec![
                b256!("0xdaa9804f41c839f316b418296d7b0ad8d91ca024d803ab632e9fd32d896f429b"),
                b256!("0x4b739f4956f43f9e2e753cecfe2569672686cba78a199684075dc494bc60b06b"),
                b256!("0x9f4fc9ded31350bade85ee54fc2d6dd8d0609fbe0f42203ab07c9a32b95fa4c4"),
            ]
        );
    }

    #[test]
    fn import() {
        let version = Version::parse("0.8.0").expect("Failed to parse version");
        let result = instrument_code(FIXTURE, "instrumentation.sol", version, LIBRARY_PATH)
            .expect("Failed to instrument");

        assert!(result
            .source
            .contains(&format!("import \"{LIBRARY_PATH}\";")));
    }

    #[test]
    fn invalid_import() {
        let version = Version::parse("0.8.0").expect("Failed to parse version");
        let result = instrument_code(
            FIXTURE,
            "instrumentation.sol",
            version,
            "\"path/with/quotes.sol\"",
        )
        .expect_err("Expected an error for invalid import path");

        assert!(matches!(
            result,
            InstrumentationError::InvalidLibraryPath { .. }
        ));
    }

    #[test]
    fn instrumentation() {
        let version = Version::parse("0.8.0").expect("Failed to parse version");
        let result = instrument_code(FIXTURE, "instrumentation.sol", version, LIBRARY_PATH)
            .expect("Failed to instrument");

        assert!(result.source.contains("__HardhatCoverage.sendHit("));
        assert!(result.source.contains(");"));
    }

    #[test]
    fn mapping() {
        let version = Version::parse("0.8.0").expect("Failed to parse version");
        let result = instrument_code(FIXTURE, "instrumentation.sol", version, LIBRARY_PATH)
            .expect("Failed to instrument");

        assert_eq!(result.metadata.len(), 3);

        assert_metadata(
            &result.metadata[0],
            b256!("0xdaa9804f41c839f316b418296d7b0ad8d91ca024d803ab632e9fd32d896f429b"),
            "    uint x = 1;\n",
        );

        assert_metadata(
            &result.metadata[1],
            b256!("0x4b739f4956f43f9e2e753cecfe2569672686cba78a199684075dc494bc60b06b"),
            "    uint y = 2;\n",
        );

        assert_metadata(
            &result.metadata[2],
            b256!("0x9f4fc9ded31350bade85ee54fc2d6dd8d0609fbe0f42203ab07c9a32b95fa4c4"),
            "    uint z = x + y;\n",
        );
    }
}
