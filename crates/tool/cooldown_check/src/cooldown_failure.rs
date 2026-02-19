use cargo_metadata::PackageId;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CooldownFailure {
    pub package_id: PackageId,
    pub name: String,
    pub current_version: String,
    pub minimum_minutes: u64,
}
