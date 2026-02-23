use cargo_metadata::{Package, PackageId};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CooldownFailure {
    pub package_id: PackageId,
    pub name: String,
    pub current_version: String,
    pub minimum_minutes: u64,
}

// TODO: find a better name for this struct
pub(crate) struct CooldownCandidate<'a> {
    pub(crate) package: &'a Package,
    pub(crate) age_minutes: u64,
    pub(crate) minimum_minutes: u64,
}

impl From<CooldownCandidate<'_>> for CooldownFailure {
    fn from(candidate: CooldownCandidate<'_>) -> Self {
        Self {
            package_id: candidate.package.id.clone(),
            name: candidate.package.name.to_string(),
            current_version: candidate.package.version.to_string(),
            minimum_minutes: candidate.minimum_minutes,
        }
    }
}
