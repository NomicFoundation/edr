//! Types related to decoding the return value of the `skip` cheatcode.

use core::fmt;

use foundry_cheatcodes_spec::constants::MAGIC_SKIP;

/// A skip reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkipReason(pub Option<String>);

impl SkipReason {
    /// Decodes a skip reason, if any.
    pub fn decode(raw_result: &[u8]) -> Option<Self> {
        raw_result.strip_prefix(MAGIC_SKIP).map(|reason| {
            let reason = String::from_utf8_lossy(reason).into_owned();
            Self((!reason.is_empty()).then_some(reason))
        })
    }

    /// Decodes a skip reason from a string that was obtained by formatting
    /// `Self`.
    ///
    /// This is a hack to support re-decoding a skip reason in proptest.
    pub fn decode_self(s: &str) -> Option<Self> {
        s.strip_prefix("skipped")
            .map(|rest| Self(rest.strip_prefix(": ").map(ToString::to_string)))
    }
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("skipped")?;
        if let Some(reason) = &self.0 {
            f.write_str(": ")?;
            f.write_str(reason)?;
        }
        Ok(())
    }
}
