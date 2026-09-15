//! The Solidity test profiles that inline configuration is resolved against.
//!
//! A directive written bare (`fuzz.runs = 3`) applies under every profile; one
//! written with a profile prefix (`ci.fuzz.runs = 8`) applies only when that
//! profile is the selected one. A prefix must name a profile the project
//! declares — validated against the *declared* set rather than the selected
//! one, so a mistyped prefix fails on every run rather than only under the
//! profile it was meant for.

use std::collections::BTreeSet;

use super::{directives::is_reserved_profile_name, error::InlineConfigProfilesError};

/// The profile that is always declared, and the one selected when the caller
/// names none.
pub const DEFAULT_PROFILE: &str = "default";

/// The profile the run was started with, together with every profile the
/// project declares.
///
/// [`Default`] is the single-profile configuration (`default` declared and
/// selected), which reproduces the behavior of a caller that knows nothing
/// about profiles.
///
/// Every declared name must be usable as a directive prefix, which
/// [`new`](Self::new) enforces: it must be non-empty, contain none of `.`, `=`,
/// or whitespace, and not collide with an inline-config key category (`fuzz`,
/// `invariant`, `isolate`, `evmVersion`, `allowInternalExpectRevert`), which
/// the parser always reads as the key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineConfigProfiles {
    selected: String,
    declared: BTreeSet<String>,
}

impl Default for InlineConfigProfiles {
    fn default() -> Self {
        Self {
            selected: DEFAULT_PROFILE.to_owned(),
            declared: BTreeSet::from([DEFAULT_PROFILE.to_owned()]),
        }
    }
}

impl InlineConfigProfiles {
    /// Constructs the profile context of a run. `default` is always added to
    /// `declared`.
    ///
    /// Fails if a declared name cannot be written as a directive prefix, or if
    /// the selected profile is not among the declared ones.
    pub fn new(
        selected: impl Into<String>,
        declared: impl IntoIterator<Item = String>,
    ) -> Result<Self, InlineConfigProfilesError> {
        let selected = selected.into();

        // A `BTreeSet` deduplicates and keeps the names sorted, so error
        // messages list them deterministically.
        let mut declared: BTreeSet<String> = declared.into_iter().collect();
        declared.insert(DEFAULT_PROFILE.to_owned());

        for name in &declared {
            validate_name(name)?;
        }

        if !declared.contains(&selected) {
            return Err(InlineConfigProfilesError::SelectedNotDeclared {
                selected,
                declared: declared.into_iter().collect(),
            });
        }

        Ok(Self { selected, declared })
    }

    /// The profile this run was started with.
    pub fn selected(&self) -> &str {
        &self.selected
    }

    /// Whether `profile` is one of the project's declared profiles.
    pub(super) fn is_declared(&self, profile: &str) -> bool {
        self.declared.contains(profile)
    }

    /// Every declared profile, sorted, for error messages.
    pub(super) fn declared_names(&self) -> Vec<String> {
        self.declared.iter().cloned().collect()
    }
}

/// Rejects a profile name that the directive parser could never read back as a
/// prefix.
fn validate_name(name: &str) -> Result<(), InlineConfigProfilesError> {
    if name.is_empty() {
        return Err(InlineConfigProfilesError::EmptyName);
    }
    if name.contains(['.', '=']) || name.chars().any(char::is_whitespace) {
        return Err(InlineConfigProfilesError::UnrepresentableName {
            name: name.to_owned(),
        });
    }
    if is_reserved_profile_name(name) {
        return Err(InlineConfigProfilesError::ReservedName {
            name: name.to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inline_config::directives::TOP_LEVEL_KEYS;

    #[test]
    fn default_selects_and_declares_the_default_profile() {
        let profiles = InlineConfigProfiles::default();

        assert_eq!(profiles.selected(), DEFAULT_PROFILE);
        assert!(profiles.is_declared(DEFAULT_PROFILE));
        assert!(!profiles.is_declared("ci"));
        assert_eq!(profiles.declared_names(), vec![DEFAULT_PROFILE.to_owned()]);
    }

    #[test]
    fn default_is_always_declared() {
        // Even when the caller doesn't list it.
        let profiles = InlineConfigProfiles::new("ci", ["ci".to_owned()]).expect("valid profiles");

        assert!(profiles.is_declared(DEFAULT_PROFILE));
        assert_eq!(
            profiles.declared_names(),
            vec!["ci".to_owned(), DEFAULT_PROFILE.to_owned()]
        );
    }

    #[test]
    fn declared_names_are_sorted_and_deduplicated() {
        let profiles = InlineConfigProfiles::new(
            DEFAULT_PROFILE,
            ["nightly".to_owned(), "ci".to_owned(), "ci".to_owned()],
        )
        .expect("valid profiles");

        assert_eq!(
            profiles.declared_names(),
            vec![
                "ci".to_owned(),
                DEFAULT_PROFILE.to_owned(),
                "nightly".to_owned()
            ]
        );
    }

    #[test]
    fn selected_must_be_declared() {
        let error = InlineConfigProfiles::new("ci", []).expect_err("undeclared selection");

        assert_eq!(
            error,
            InlineConfigProfilesError::SelectedNotDeclared {
                selected: "ci".to_owned(),
                declared: vec![DEFAULT_PROFILE.to_owned()],
            }
        );
        assert!(error.to_string().contains("declared profiles are: default"));
    }

    #[test]
    fn empty_name_is_rejected() {
        let error =
            InlineConfigProfiles::new(DEFAULT_PROFILE, [String::new()]).expect_err("empty name");

        assert_eq!(error, InlineConfigProfilesError::EmptyName);
    }

    #[test]
    fn names_the_parser_cannot_split_are_rejected() {
        // The parser splits a key on the first `=`, trims it, then splits off
        // the prefix at the first `.`; a name containing any of those could
        // never be matched.
        for name in ["my.ci", "ci=1", "my ci", " ci", "ci\t"] {
            let error =
                InlineConfigProfiles::new(DEFAULT_PROFILE, [name.to_owned()]).expect_err(name);

            assert_eq!(
                error,
                InlineConfigProfilesError::UnrepresentableName {
                    name: name.to_owned(),
                },
                "{name:?}"
            );
        }
    }

    #[test]
    fn key_categories_are_reserved() {
        for name in TOP_LEVEL_KEYS {
            let error =
                InlineConfigProfiles::new(DEFAULT_PROFILE, [name.to_owned()]).expect_err(name);

            assert_eq!(
                error,
                InlineConfigProfilesError::ReservedName {
                    name: name.to_owned(),
                },
                "{name}"
            );
        }
    }

    #[test]
    fn invalid_selected_name_is_reported_as_invalid_not_undeclared() {
        let error = InlineConfigProfiles::new("fuzz", ["fuzz".to_owned()]).expect_err("reserved");

        assert_eq!(
            error,
            InlineConfigProfilesError::ReservedName {
                name: "fuzz".to_owned(),
            }
        );
    }
}
