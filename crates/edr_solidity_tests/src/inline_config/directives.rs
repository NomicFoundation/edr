//! Parsing and validation of inline-config directives.
//!
//! Directives live in NatSpec comments above test functions and use either the
//! `forge-config:` or `hardhat-config:` prefix, e.g.:
//!
//! ```text
//! forge-config: default.fuzz.runs = 100
//! hardhat-config: invariant.fail-on-revert = true
//! ```

use super::{error::InlineConfigError, natspec::NatSpecBlock};
use crate::config::{TestFunctionConfigOverride, TimeoutConfig};

const HARDHAT_CONFIG_PREFIX: &str = "hardhat-config:";
const FORGE_CONFIG_PREFIX: &str = "forge-config:";

/// A candidate directive line, stripped of comment decoration, together with
/// the byte offset of the source line it came from (for line-number reporting).
struct DirectiveLine<'a> {
    /// Byte offset within the source of the line this came from.
    offset: usize,
    /// The line text, stripped of comment decoration.
    text: &'a str,
}

/// A directive problem together with the byte offset of the offending line, so
/// the collection layer can resolve it to a source line number.
#[derive(Debug)]
pub(super) struct LocatedDirectiveError {
    /// Byte offset within the source of the offending directive.
    pub(super) offset: usize,
    /// The problem.
    pub(super) error: InlineConfigError,
}

/// Top-level inline-config key categories. A leading dot-segment that is not
/// one of these is interpreted as a (profile) prefix.
const TOP_LEVEL_KEYS: [&str; 5] = [
    "fuzz",
    "invariant",
    "allowInternalExpectRevert",
    "isolate",
    "evmVersion",
];

/// Inline-config profiles the parser accepts as a leading dot-segment prefix.
/// Only `default` is supported today; add new profiles here to extend support.
const SUPPORTED_PROFILES: [&str; 1] = ["default"];

/// Whether `name` is an invariant test, matching the runner's classification
/// (`invariant*` or the `statefulFuzz*` alias).
pub(super) fn is_invariant_function(name: &str) -> bool {
    name.starts_with("invariant") || name.starts_with("statefulFuzz")
}

/// Whether `name` is a function the runner treats as a test, and which may
/// therefore carry inline configuration.
pub(super) fn is_test_function(name: &str) -> bool {
    name.starts_with("test") || is_invariant_function(name)
}

/// The kind of test a key applies to. A key is rejected when it appears on a
/// test of a different kind; [`KeyCategory::Any`] keys are valid on both.
#[derive(Clone, Copy, PartialEq, Eq)]
enum KeyCategory {
    /// Valid on any test (e.g. `isolate`, `evmVersion`).
    Any,
    /// Only valid on fuzz tests (`fuzz.*`).
    Fuzz,
    /// Only valid on invariant tests (`invariant.*`).
    Invariant,
}

/// A recognized inline-config key.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    Isolate,
    AllowInternalExpectRevert,
    EvmVersion,
    FuzzRuns,
    FuzzMaxTestRejects,
    FuzzShowLogs,
    FuzzTimeout,
    InvariantRuns,
    InvariantDepth,
    InvariantFailOnRevert,
    InvariantCallOverride,
    InvariantTimeout,
}

impl Key {
    /// Parses a canonical (camelCase) key, returning `None` if it is unknown.
    fn from_canonical(key: &str) -> Option<Self> {
        let key = match key {
            "isolate" => Self::Isolate,
            "allowInternalExpectRevert" => Self::AllowInternalExpectRevert,
            "evmVersion" => Self::EvmVersion,
            "fuzz.runs" => Self::FuzzRuns,
            "fuzz.maxTestRejects" => Self::FuzzMaxTestRejects,
            "fuzz.showLogs" => Self::FuzzShowLogs,
            "fuzz.timeout" => Self::FuzzTimeout,
            "invariant.runs" => Self::InvariantRuns,
            "invariant.depth" => Self::InvariantDepth,
            "invariant.failOnRevert" => Self::InvariantFailOnRevert,
            "invariant.callOverride" => Self::InvariantCallOverride,
            "invariant.timeout" => Self::InvariantTimeout,
            _ => return None,
        };
        Some(key)
    }

    /// The kind of test this key may appear on.
    fn category(self) -> KeyCategory {
        match self {
            Self::Isolate | Self::AllowInternalExpectRevert | Self::EvmVersion => KeyCategory::Any,
            Self::FuzzRuns | Self::FuzzMaxTestRejects | Self::FuzzShowLogs | Self::FuzzTimeout => {
                KeyCategory::Fuzz
            }
            Self::InvariantRuns
            | Self::InvariantDepth
            | Self::InvariantFailOnRevert
            | Self::InvariantCallOverride
            | Self::InvariantTimeout => KeyCategory::Invariant,
        }
    }

    /// Validates `raw`'s value against this key's expected type and writes it
    /// into `config`.
    fn apply(
        self,
        config: &mut TestFunctionConfigOverride,
        raw: &RawOverride,
    ) -> Result<(), InlineConfigError> {
        let invalid_value = |expected: &'static str| InlineConfigError::InvalidValue {
            key: raw.raw_key.clone(),
            value: raw.raw_value.clone(),
            expected,
        };

        let as_bool = || {
            if raw.raw_value.eq_ignore_ascii_case("true") {
                Ok(true)
            } else if raw.raw_value.eq_ignore_ascii_case("false") {
                Ok(false)
            } else {
                Err(invalid_value("boolean"))
            }
        };
        let as_u32 = || {
            if is_non_negative_integer(&raw.raw_value) {
                parse_u32(&raw.raw_value, &raw.raw_key)
            } else {
                Err(invalid_value("non-negative integer"))
            }
        };
        let as_string = || {
            // The surrounding quotes are stripped once validated.
            if is_quoted_string(&raw.raw_value) {
                Ok(raw
                    .raw_value
                    .get(1..raw.raw_value.len() - 1)
                    .unwrap_or_default()
                    .to_owned())
            } else {
                Err(invalid_value("non-empty double-quoted string"))
            }
        };

        match self {
            Self::Isolate => config.isolate = Some(as_bool()?),
            Self::AllowInternalExpectRevert => {
                config.allow_internal_expect_revert = Some(as_bool()?);
            }
            Self::EvmVersion => config.evm_version = Some(as_string()?),
            Self::FuzzRuns => config.fuzz.get_or_insert_default().runs = Some(as_u32()?),
            Self::FuzzMaxTestRejects => {
                config.fuzz.get_or_insert_default().max_test_rejects = Some(as_u32()?);
            }
            Self::FuzzShowLogs => config.fuzz.get_or_insert_default().show_logs = Some(as_bool()?),
            Self::FuzzTimeout => {
                config.fuzz.get_or_insert_default().timeout = Some(TimeoutConfig {
                    time: Some(as_u32()?),
                });
            }
            Self::InvariantRuns => config.invariant.get_or_insert_default().runs = Some(as_u32()?),
            Self::InvariantDepth => {
                config.invariant.get_or_insert_default().depth = Some(as_u32()?);
            }
            Self::InvariantFailOnRevert => {
                config.invariant.get_or_insert_default().fail_on_revert = Some(as_bool()?);
            }
            Self::InvariantCallOverride => {
                config.invariant.get_or_insert_default().call_override = Some(as_bool()?);
            }
            Self::InvariantTimeout => {
                config.invariant.get_or_insert_default().timeout = Some(TimeoutConfig {
                    time: Some(as_u32()?),
                });
            }
        }

        Ok(())
    }
}

/// A single parsed directive, prior to validation.
struct RawOverride {
    /// Canonical (camelCase) key.
    key: String,
    /// The key exactly as written (for diagnostics).
    raw_key: String,
    /// The value exactly as written.
    raw_value: String,
    /// Byte offset within the source of the directive's line.
    offset: usize,
}

/// Replaces each `delim` followed by a character with the uppercased character
/// (`max-test-rejects` and `max_test_rejects` both become `maxTestRejects`).
/// A trailing delimiter with no following character is left untouched to avoid
/// silently fixing malformed keys.
fn delimiter_to_camel(input: &str, delim: char) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == delim {
            match chars.next() {
                Some(next) => out.extend(next.to_uppercase()),
                None => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Splits a scanned comment block into candidate directive lines, stripping
/// both the block delimiters (`///`, `/**`, `*/`) and each line's leading
/// NatSpec decoration (whitespace, an optional `*`, then whitespace). Each line
/// keeps the byte offset of the source line it came from.
fn block_to_lines(block: &NatSpecBlock) -> Vec<DirectiveLine<'_>> {
    fn strip_decoration(line: &str) -> &str {
        let line = line.trim_start();
        let line = line.strip_prefix("///").unwrap_or(line);
        let line = line.strip_prefix("/**").unwrap_or(line);
        let line = line.strip_suffix("*/").unwrap_or(line);
        let line = line.trim_start();
        let line = line.strip_prefix('*').unwrap_or(line);
        // A block comment's closing `*/` can trail a directive on its own line.
        let line = line.strip_suffix("*/").unwrap_or(line);
        line.trim()
    }

    // Walk the block's physical lines, tracking each one's byte offset within
    // the source (`block.offset` is the block's start). A `///` block is a
    // single line whose stored text was left-trimmed by the scanner, so its
    // offset is simply the block offset.
    let mut lines = Vec::new();
    let mut line_start = 0;
    for physical in block.text.split_inclusive('\n') {
        lines.push(DirectiveLine {
            offset: block.offset + line_start,
            text: strip_decoration(physical),
        });
        line_start += physical.len();
    }
    lines
}

/// Parses a single candidate directive line — already stripped of comment
/// decoration by [`block_to_lines`] — returning `None` if it is not an inline
/// config directive.
fn parse_line(line: &DirectiveLine<'_>) -> Result<Option<RawOverride>, LocatedDirectiveError> {
    let text = line.text;
    let located = |error: InlineConfigError| LocatedDirectiveError {
        offset: line.offset,
        error,
    };

    let segment = if let Some(rest) = text.strip_prefix(HARDHAT_CONFIG_PREFIX) {
        rest.trim()
    } else if let Some(rest) = text.strip_prefix(FORGE_CONFIG_PREFIX) {
        rest.trim()
    } else {
        return Ok(None);
    };

    let Some((raw_key, raw_value)) = segment.split_once('=') else {
        return Err(located(InlineConfigError::InvalidSyntax {
            line: text.to_owned(),
        }));
    };
    let raw_key = raw_key.trim();
    let raw_value = raw_value.trim();

    // Detect and strip a profile prefix (see `SUPPORTED_PROFILES`).
    let mut key = raw_key;
    if let Some((first_segment, rest)) = raw_key.split_once('.')
        && !TOP_LEVEL_KEYS.contains(&first_segment)
    {
        if !SUPPORTED_PROFILES.contains(&first_segment) {
            return Err(located(InlineConfigError::UnsupportedProfile {
                profile: first_segment.to_owned(),
            }));
        }
        key = rest;
    }

    let key = delimiter_to_camel(&delimiter_to_camel(key, '-'), '_');

    Ok(Some(RawOverride {
        key,
        raw_key: raw_key.to_owned(),
        raw_value: raw_value.to_owned(),
        offset: line.offset,
    }))
}

/// Returns `true` if `value` is a non-negative decimal integer with no leading
/// zeros.
fn is_non_negative_integer(value: &str) -> bool {
    match value.as_bytes() {
        [b'0'] => true,
        [first, ..] if *first != b'0' => value.bytes().all(|byte| byte.is_ascii_digit()),
        _ => false,
    }
}

/// Returns `true` if `value` is a non-empty double-quoted string with no
/// embedded quote or newline.
fn is_quoted_string(value: &str) -> bool {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .is_some_and(|inner| !inner.is_empty() && !inner.contains(['"', '\n']))
}

/// Parses a validated non-negative integer into a `u32`.
fn parse_u32(value: &str, raw_key: &str) -> Result<u32, InlineConfigError> {
    value
        .parse::<u32>()
        .map_err(|_error| InlineConfigError::InvalidValue {
            key: raw_key.to_owned(),
            value: value.to_owned(),
            expected: "non-negative integer",
        })
}

/// Parses the inline configuration for a single function from its leading
/// NatSpec blocks.
///
/// Returns `Ok(None)` when no inline-config directive is present. On a
/// malformed directive, the error carries the byte offset of the offending line
/// so the caller can resolve it to a source line number.
pub(super) fn parse_inline_config(
    blocks: &[NatSpecBlock],
    function: &str,
) -> Result<Option<TestFunctionConfigOverride>, LocatedDirectiveError> {
    let mut raw_overrides = Vec::new();
    for block in blocks {
        for line in block_to_lines(block) {
            if let Some(raw) = parse_line(&line)? {
                raw_overrides.push(raw);
            }
        }
    }

    if raw_overrides.is_empty() {
        return Ok(None);
    }

    let is_fuzz_test = function.starts_with("test");
    let is_invariant_test = is_invariant_function(function);

    let mut config = TestFunctionConfigOverride::default();
    let mut seen = Vec::new();

    for raw in &raw_overrides {
        let located = |error: InlineConfigError| LocatedDirectiveError {
            offset: raw.offset,
            error,
        };

        let Some(key) = Key::from_canonical(&raw.key) else {
            return Err(located(InlineConfigError::InvalidKey {
                key: raw.raw_key.clone(),
            }));
        };

        // Key must match the test kind; top-level keys are valid on both.
        let valid_for_kind = match key.category() {
            KeyCategory::Any => true,
            KeyCategory::Fuzz => is_fuzz_test,
            KeyCategory::Invariant => is_invariant_test,
        };
        if !valid_for_kind {
            return Err(located(InlineConfigError::InvalidKeyForTestType {
                key: raw.raw_key.clone(),
                test_type: if is_fuzz_test { "fuzz" } else { "invariant" }.to_owned(),
            }));
        }

        // Reject duplicate keys for the same function.
        if seen.contains(&key) {
            return Err(located(InlineConfigError::DuplicateKey {
                key: raw.raw_key.clone(),
            }));
        }
        seen.push(key);

        key.apply(&mut config, raw).map_err(located)?;
    }

    Ok(Some(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(text: &str) -> NatSpecBlock {
        NatSpecBlock {
            text: text.to_owned(),
            offset: 0,
        }
    }

    fn parse(
        text: &str,
        function: &str,
    ) -> Result<Option<TestFunctionConfigOverride>, InlineConfigError> {
        parse_inline_config(&[block(text)], function).map_err(|located| located.error)
    }

    #[test]
    fn no_directive_returns_none() {
        assert_eq!(parse("/// @notice hello", "testFoo"), Ok(None));
    }

    #[test]
    fn forge_config_with_default_profile() {
        let cfg = parse("/// forge-config: default.fuzz.runs = 100", "testFoo")
            .unwrap()
            .unwrap();
        assert_eq!(cfg.fuzz.unwrap().runs, Some(100));
    }

    #[test]
    fn hardhat_config_without_profile() {
        let cfg = parse("/// hardhat-config: fuzz.runs = 100", "testFoo")
            .unwrap()
            .unwrap();
        assert_eq!(cfg.fuzz.unwrap().runs, Some(100));
    }

    #[test]
    fn kebab_snake_camel_equivalence() {
        for spelling in [
            "fuzz.max-test-rejects",
            "fuzz.max_test_rejects",
            "fuzz.maxTestRejects",
        ] {
            let cfg = parse(&format!("/// forge-config: {spelling} = 7"), "testFoo")
                .unwrap()
                .unwrap();
            assert_eq!(cfg.fuzz.unwrap().max_test_rejects, Some(7), "{spelling}");
        }
    }

    #[test]
    fn block_comment_multiple_keys() {
        let text = "/**\n * forge-config: default.invariant.runs = 256\n * hardhat-config: invariant.fail-on-revert = true\n */";
        let cfg = parse(text, "invariant_balance").unwrap().unwrap();
        let inv = cfg.invariant.unwrap();
        assert_eq!(inv.runs, Some(256));
        assert_eq!(inv.fail_on_revert, Some(true));
    }

    #[test]
    fn block_comment_single_line() {
        let cfg = parse("/** forge-config: default.fuzz.runs = 100 */", "testFoo")
            .unwrap()
            .unwrap();
        assert_eq!(cfg.fuzz.unwrap().runs, Some(100));
    }

    #[test]
    fn block_comment_without_leading_stars() {
        // The per-line `*` is optional; a block whose lines have none is still
        // parsed.
        let text = "/**\nforge-config: fuzz.runs = 5\nhardhat-config: fuzz.show-logs = true\n*/";
        let cfg = parse(text, "testFoo").unwrap().unwrap();
        let fuzz = cfg.fuzz.unwrap();
        assert_eq!(fuzz.runs, Some(5));
        assert_eq!(fuzz.show_logs, Some(true));
    }

    #[test]
    fn block_comment_surfaces_errors() {
        // Validation applies to directives in block comments just as for `///`.
        let err = parse("/** forge-config: fuzz.runs = -1 */", "testFoo").unwrap_err();
        assert!(matches!(err, InlineConfigError::InvalidValue { .. }));

        let err = parse("/**\n * forge-config: fuzz.bogus = 1\n */", "testFoo").unwrap_err();
        assert!(matches!(err, InlineConfigError::InvalidKey { .. }));
    }

    #[test]
    fn top_level_keys() {
        let cfg = parse_inline_config(
            &[
                block("/// hardhat-config: isolate = true"),
                block("/// hardhat-config: evmVersion = \"cancun\""),
                block("/// hardhat-config: allow-internal-expect-revert = true"),
            ],
            "testFoo",
        )
        .unwrap()
        .unwrap();
        assert_eq!(cfg.isolate, Some(true));
        assert_eq!(cfg.evm_version.as_deref(), Some("cancun"));
        assert_eq!(cfg.allow_internal_expect_revert, Some(true));
    }

    #[test]
    fn invalid_syntax_missing_equals() {
        let err = parse("/// forge-config: default.fuzz.runs 100", "testFoo").unwrap_err();
        assert!(matches!(err, InlineConfigError::InvalidSyntax { .. }));
    }

    #[test]
    fn unsupported_profile() {
        let err = parse("/// forge-config: ci.fuzz.runs = 100", "testFoo").unwrap_err();
        assert!(matches!(err, InlineConfigError::UnsupportedProfile { .. }));
    }

    #[test]
    fn invalid_key() {
        let err = parse("/// forge-config: fuzz.bogus = 1", "testFoo").unwrap_err();
        assert!(matches!(err, InlineConfigError::InvalidKey { .. }));
    }

    #[test]
    fn invalid_key_for_test_type() {
        let err = parse("/// forge-config: fuzz.runs = 1", "invariant_x").unwrap_err();
        assert!(matches!(
            err,
            InlineConfigError::InvalidKeyForTestType { .. }
        ));
        let err = parse("/// forge-config: invariant.runs = 1", "testFoo").unwrap_err();
        assert!(matches!(
            err,
            InlineConfigError::InvalidKeyForTestType { .. }
        ));
    }

    #[test]
    fn top_level_keys_allowed_on_both() {
        assert!(parse("/// forge-config: isolate = true", "invariant_x")
            .unwrap()
            .is_some());
    }

    /// `parse_line`'s profile detection treats a leading dot-segment as a
    /// profile unless it is a known key prefix in `TOP_LEVEL_KEYS`. If a new
    /// key category were added but not to `TOP_LEVEL_KEYS`, its keys would
    /// be silently misread as an (unsupported) profile. This guards that.
    #[test]
    fn top_level_keys_cover_every_key_leading_segment() {
        // Every canonical key the parser recognizes (keep in sync with `Key`).
        let canonical_keys = [
            "isolate",
            "allowInternalExpectRevert",
            "evmVersion",
            "fuzz.runs",
            "fuzz.maxTestRejects",
            "fuzz.showLogs",
            "fuzz.timeout",
            "invariant.runs",
            "invariant.depth",
            "invariant.failOnRevert",
            "invariant.callOverride",
            "invariant.timeout",
        ];

        for key in canonical_keys {
            assert!(
                Key::from_canonical(key).is_some(),
                "`{key}` should be a known key"
            );

            let leading = key.split('.').next().expect("canonical key is non-empty");
            assert!(
                TOP_LEVEL_KEYS.contains(&leading),
                "TOP_LEVEL_KEYS is missing `{leading}` (leading segment of `{key}`)"
            );
        }
    }

    #[test]
    fn invalid_number_value() {
        for bad in ["-1", "1.5", "01", "0x10", "true"] {
            let err =
                parse(&format!("/// forge-config: fuzz.runs = {bad}"), "testFoo").unwrap_err();
            assert!(
                matches!(err, InlineConfigError::InvalidValue { .. }),
                "{bad}"
            );
        }
        assert!(parse("/// forge-config: fuzz.runs = 0", "testFoo")
            .unwrap()
            .is_some());
    }

    #[test]
    fn invalid_boolean_value() {
        let err = parse("/// forge-config: isolate = yes", "testFoo").unwrap_err();
        assert!(matches!(err, InlineConfigError::InvalidValue { .. }));
    }

    #[test]
    fn invalid_string_value() {
        for bad in ["cancun", "\"\"", "\"a"] {
            let err =
                parse(&format!("/// forge-config: evmVersion = {bad}"), "testFoo").unwrap_err();
            assert!(
                matches!(err, InlineConfigError::InvalidValue { .. }),
                "{bad}"
            );
        }
    }

    #[test]
    fn duplicate_key() {
        let err = parse_inline_config(
            &[
                block("/// forge-config: fuzz.runs = 1"),
                block("/// forge-config: fuzz.runs = 2"),
            ],
            "testFoo",
        )
        .unwrap_err()
        .error;
        assert!(matches!(err, InlineConfigError::DuplicateKey { .. }));
    }

    #[test]
    fn timeout_parsed_as_seconds() {
        let cfg = parse("/// forge-config: fuzz.timeout = 30", "testFoo")
            .unwrap()
            .unwrap();
        assert_eq!(
            cfg.fuzz.unwrap().timeout,
            Some(TimeoutConfig { time: Some(30) })
        );
    }
}
