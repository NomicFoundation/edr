//! Extraction of leading NatSpec comment blocks directly from source text.
//!
//! Slang does not attach documentation comments to syntax nodes, so we recover
//! them from the raw source (see [`super::parse`]). Given a function's
//! start offset, we scan *backwards* from the function: the scan stops at the
//! first byte that is neither whitespace nor part of a comment, so it reads
//! only the leading comments immediately above the function and never the rest
//! of the contract.
//!
//! # Which comments count
//!
//! *All* NatSpec blocks (`///` runs and `/** */` comments) in the function's
//! leading comment region are collected, in source order. Plain `//` and
//! `/* */` comments and blank lines in that region are transparent: they
//! neither terminate the scan nor split off the NatSpec blocks around them.
//!
//! This matches the parser Foundry uses (Solar), whose lexer discards plain
//! comments and whitespace as trivia and attaches every preceding doc comment
//! to the item. It is deliberately *more* permissive than the solc AST's
//! `documentation` field: solc treats a plain comment or a blank line between
//! two NatSpec blocks as a break and keeps only the last block before the
//! declaration. Directives in an earlier block are therefore honored here and
//! by Foundry, but are invisible in solc build info. In the common cases —
//! one NatSpec block, possibly with plain comments between it and the
//! function — all three agree.

/// A NatSpec comment block found in a function's leading region.
///
/// Either a single `/** ... */` block comment or a single `///` line comment.
/// The [`text`](NatSpecBlock::text) still contains the comment delimiters;
/// callers strip them per line (see [`super::directives`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NatSpecBlock {
    /// The raw block text, delimiters included.
    pub text: String,
    /// Byte offset of the block within the source, used to report the line of
    /// an offending directive.
    pub offset: usize,
}

/// Returns the NatSpec comment blocks immediately preceding the node starting
/// at `node_start`, in source order.
///
/// Scanning runs backwards from `node_start` and stops at the first byte that
/// is neither whitespace nor part of a comment — so it terminates at the
/// previous member's `}`/`;` (or the enclosing contract's `{`), and the work is
/// bounded by the size of the leading comments, not the rest of the source.
///
/// Plain `//` and `/* */` comments are skipped like whitespace rather than
/// terminating the scan. Only `///` and `/** */` text is collected.
pub fn collect_natspec(src: &str, node_start: usize) -> Vec<NatSpecBlock> {
    let Some(region) = src.get(..node_start) else {
        return Vec::new();
    };

    let mut blocks = Vec::new();
    // `end` is a byte offset within `region`; we consume comments from the end.
    let mut end = region.len();

    loop {
        // Skip whitespace between the node/previous block and the next comment.
        let head = region.get(..end).unwrap_or("");
        let trimmed = head.trim_end();
        end = trimmed.len();
        if end == 0 {
            break;
        }

        if trimmed.ends_with("*/") {
            // Block comment: find its opening `/*`.
            let Some(start) = trimmed.rfind("/*") else {
                break;
            };
            let block = region.get(start..end).unwrap_or("");
            // Only `/** */` is NatSpec; a plain `/* */` is skipped.
            if block.starts_with("/**") {
                blocks.push(NatSpecBlock {
                    text: block.to_owned(),
                    offset: start,
                });
            }
            end = start;
        } else {
            // Possibly the last line of a comment run. Locate the line start.
            let line_start = region
                .get(..end)
                .and_then(|head| head.rfind('\n').map(|index| index + 1))
                .unwrap_or(0);
            let line = region.get(line_start..end).unwrap_or("");
            let content = line.trim_start();
            // Only `///` is NatSpec; a plain `//` comment is skipped; anything
            // else is code and ends the scan.
            if content.starts_with("///") {
                blocks.push(NatSpecBlock {
                    text: content.to_owned(),
                    offset: line_start,
                });
            } else if !content.starts_with("//") {
                break;
            }
            end = line_start;
        }
    }

    blocks.reverse();
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scans backwards from the `function` keyword — mirroring how
    /// `parse` calls it.
    fn scan(src: &str) -> Vec<String> {
        let node_start = src.find("function ").expect("needs a function");
        collect_natspec(src, node_start)
            .into_iter()
            .map(|block| block.text)
            .collect()
    }

    #[test]
    fn single_line_natspec() {
        let src =
            "contract C {\n    /// forge-config: default.fuzz.runs = 5\n    function f() {}\n}";
        assert_eq!(
            scan(src),
            vec!["/// forge-config: default.fuzz.runs = 5".to_owned()]
        );
    }

    #[test]
    fn consecutive_line_natspec_in_source_order() {
        let src = "contract C {\n    /// @notice hi\n    /// forge-config: default.fuzz.runs = 5\n    function f() {}\n}";
        assert_eq!(
            scan(src),
            vec![
                "/// @notice hi".to_owned(),
                "/// forge-config: default.fuzz.runs = 5".to_owned(),
            ]
        );
    }

    #[test]
    fn block_natspec() {
        let src = "contract C {\n    /**\n     * forge-config: default.fuzz.runs = 5\n     */\n    function f() {}\n}";
        let blocks = scan(src);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].starts_with("/**"));
        assert!(blocks[0].contains("forge-config"));
    }

    #[test]
    fn ignores_plain_line_comments() {
        // A `//` (non-doc) comment is skipped without being collected; the
        // `///` directive below it is.
        let src = "contract C {\n    // regular\n    /// forge-config: default.fuzz.runs = 5\n    function f() {}\n}";
        assert_eq!(
            scan(src),
            vec!["/// forge-config: default.fuzz.runs = 5".to_owned()]
        );
    }

    #[test]
    fn collects_across_plain_line_comments() {
        // solc attaches a doc comment to the following definition across
        // intervening plain comments, so the directive above the `//` lines
        // must still be collected. Mirrors uniswap-v4-core's
        // `ModifyLiquidity.t.sol`, where non-default profile directives are
        // commented out between the NatSpec directive and the function.
        let src = "contract C {\n    /// forge-config: default.fuzz.runs = 10\n    // forge-config: pr.fuzz.runs = 10 // HARDHAT: non-default profiles not supported inline\n    // forge-config: ci.fuzz.runs = 500 // HARDHAT: non-default profiles not supported inline\n    // forge-config: debug.fuzz.runs = 10 // HARDHAT: non-default profiles not supported inline\n    function f() {}\n}";
        assert_eq!(
            scan(src),
            vec!["/// forge-config: default.fuzz.runs = 10".to_owned()]
        );
    }

    #[test]
    fn collects_both_blocks_split_by_plain_comment() {
        // A plain comment between two NatSpec blocks splits them, but both are
        // still collected (Solar semantics). The solc AST would keep only the
        // last block — see the module docs.
        let src = "contract C {\n    /// forge-config: default.fuzz.runs = 5\n    // regular\n    /// forge-config: default.fuzz.max-test-rejects = 7\n    function f() {}\n}";
        assert_eq!(
            scan(src),
            vec![
                "/// forge-config: default.fuzz.runs = 5".to_owned(),
                "/// forge-config: default.fuzz.max-test-rejects = 7".to_owned(),
            ]
        );
    }

    #[test]
    fn collects_both_blocks_split_by_blank_line() {
        // A blank line also splits NatSpec blocks in solc (only the last would
        // survive in build info); both are collected here, matching Solar.
        let src = "contract C {\n    /// forge-config: default.fuzz.runs = 5\n\n    /// forge-config: default.fuzz.max-test-rejects = 7\n    function f() {}\n}";
        assert_eq!(
            scan(src),
            vec![
                "/// forge-config: default.fuzz.runs = 5".to_owned(),
                "/// forge-config: default.fuzz.max-test-rejects = 7".to_owned(),
            ]
        );
    }

    #[test]
    fn collects_across_plain_block_comments() {
        let src = "contract C {\n    /// forge-config: default.fuzz.runs = 5\n    /* plain */\n    function f() {}\n}";
        assert_eq!(
            scan(src),
            vec!["/// forge-config: default.fuzz.runs = 5".to_owned()]
        );
    }

    #[test]
    fn stops_at_previous_member() {
        // Code between the directive and an earlier comment terminates the
        // scan, so the earlier `/// detached` comment does not leak in.
        let src = "contract C {\n    /// detached\n    uint256 x;\n\n    /// forge-config: default.fuzz.runs = 5\n    function f() {}\n}";
        assert_eq!(
            scan(src),
            vec!["/// forge-config: default.fuzz.runs = 5".to_owned()]
        );
    }

    #[test]
    fn plain_comments_alone_yield_nothing() {
        let src = "contract C {\n    /* not natspec */\n    function f() {}\n}";
        assert!(scan(src).is_empty());

        let src = "contract C {\n    // not natspec\n    function f() {}\n}";
        assert!(scan(src).is_empty());
    }

    #[test]
    fn no_comment() {
        let src = "contract C {\n    function f() {}\n}";
        assert!(scan(src).is_empty());
    }

    #[test]
    fn empty_region() {
        let src = "contract C { function f() {} }";
        let node_start = src.find("function ").unwrap();
        assert!(collect_natspec(src, node_start).is_empty());
    }
}
