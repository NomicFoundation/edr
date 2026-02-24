use cargo_metadata::{Package, PackageId};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CooldownFailure {
    pub package_id: PackageId,
    pub name: String,
    pub current_version: String,
    pub age_threshold_minutes: u64,
}

pub(crate) struct ResolvedAge<'a> {
    pub(crate) package: &'a Package,
    pub(crate) age_minutes: u64,
    pub(crate) age_threshold_minutes: u64,
}

impl From<ResolvedAge<'_>> for CooldownFailure {
    fn from(resolved: ResolvedAge<'_>) -> Self {
        Self {
            package_id: resolved.package.id.clone(),
            name: resolved.package.name.to_string(),
            current_version: resolved.package.version.to_string(),
            age_threshold_minutes: resolved.age_threshold_minutes,
        }
    }
}
