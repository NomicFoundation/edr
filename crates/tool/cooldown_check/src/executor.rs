use std::collections::{HashMap, HashSet};

use anyhow::{bail, Result};
use cargo_metadata::{Dependency, Package, PackageId};
use futures::stream::{self, StreamExt};
use semver::VersionReq;

use crate::{
    allowlist::Allowlist,
    config::Config,
    resolver::Resolver,
    types::{CooldownCandidate, CooldownFailure},
    workspace::Workspace,
};

const MAX_CONCURRENT_FETCHES: usize = 10;

pub async fn run_check_flow(workspace: Workspace) -> Result<()> {
    ensure_lockfile(&workspace)?;

    let allowlist = &workspace.allowlist;
    let config = &workspace.config;
    let packages = workspace.packages();

    if config.cooldown_minutes == 0 {
        log::info!("Skipping cooldown check: cooldown minutes is set to 0");
        return Ok(());
    }

    let resolver = &Resolver::new(config)?;

    // Filter packages that need an age check.
    let dependencies_to_validate = workspace.nodes.iter().filter_map(|node| {
        let package = packages
            .get(&node.id)
            .unwrap_or_else(|| panic!("Could not find associated package to {:?}", node.id));
        cooldown_requirement(package, config, allowlist)
    });

    let cooldown_candidates =
        resolve_cooldown_candidates(dependencies_to_validate, resolver).await?;

    let cooldown_failures = cooldown_candidates
        .into_iter()
        .filter_map(|candidate| detect_cooldown_failure(candidate))
        .collect::<HashSet<_>>();

    if cooldown_failures.is_empty() {
        log::info!("Dependency graph passed cooldown check ✅");
        Ok(())
    } else {
        report_cooldown_failures(&workspace, resolver, cooldown_failures).await?;
        bail!("dependency graph failed cooldown check ❌")
    }
}

/// Returns the cooldown requirement for a package, or `None` if the package
/// is exempt (local dependency, non-allowed registry, zero-minute cooldown, or
/// exact-version allowlisted).
fn cooldown_requirement<'a>(
    package: &'a Package,
    config: &Config,
    allowlist: &Allowlist,
) -> Option<(&'a Package, u64)> {
    if package.source.is_none() {
        log::debug!(
            "Skipping validation for crate {}@{}: crate is a local dependency",
            package.name,
            package.version
        );
        return None;
    }

    if package
        .source
        .as_ref()
        .is_some_and(|source| !config.is_registry_allowed(&source.repr))
    {
        log::warn!(
            "Skipping non-crates.io registry dependency. crate = {}, source = {}",
            package.name,
            package
                .source
                .as_ref()
                .map(|source| &source.repr)
                .expect("Source should be present")
        );
        return None;
    }

    let minimum_minutes = allowlist
        .crate_minutes(package.name.as_str())
        .map_or(config.cooldown_minutes, |minutes| {
            config.cooldown_minutes.min(minutes)
        });
    let exact_allowed =
        allowlist.is_exact_allowed(package.name.as_str(), &package.version.to_string());

    if minimum_minutes == 0 {
        log::info!("Skipping validation for crate {}@{}: `allow.package.minutes` is set to 0 in the allowlist", package.name, package.version);
        return None;
    }
    if exact_allowed {
        log::info!(
            "Skipping validation for crate {}@{}: version is listed as `allow.exact`",
            package.name,
            package.version
        );
        return None;
    }
    Some((package, minimum_minutes))
}

/// Concurrently resolves each dependency into a [`CooldownCandidate`] by
/// fetching its published age. Bails on the first fetch error.
async fn resolve_cooldown_candidates<'a>(
    dependencies: impl Iterator<Item = (&'a Package, u64)>,
    resolver: &Resolver,
) -> Result<Vec<CooldownCandidate<'a>>> {
    let mut stream = stream::iter(dependencies)
        .map(|(package, minimum_minutes)| async move {
            let age = resolver
                .fetch_version_age(package.name.as_str(), &package.version.to_string())
                .await;
            (package, age, minimum_minutes)
        })
        .buffer_unordered(MAX_CONCURRENT_FETCHES);

    let mut candidates: Vec<CooldownCandidate<'_>> = Vec::new();
    while let Some((package, age_result, minimum_minutes)) = stream.next().await {
        let age_minutes = age_result?;
        candidates.push(CooldownCandidate {
            package,
            age_minutes,
            minimum_minutes,
        });
    }
    Ok(candidates)
}

async fn report_cooldown_failures(
    workspace: &Workspace,
    resolver: &Resolver,
    cooldown_failures: HashSet<CooldownFailure>,
) -> anyhow::Result<()> {
    let failing_crate_names = cooldown_failures
        .iter()
        .map(|failure| failure.name.clone())
        .collect::<Vec<_>>();

    let version_requirements = gather_dependencies_requirements(failing_crate_names, workspace);

    for failure in cooldown_failures {
        let crate_requirements = version_requirements
            .get(&failure.package_id)
            .map_or_else(Vec::new, |requirements| {
                requirements.iter().cloned().collect::<Vec<_>>()
            });
        let version_candidates = resolver
            .find_version_candidates(&failure, &crate_requirements)
            .await?
            .into_iter()
            .collect::<Vec<_>>();
        if version_candidates.is_empty() {
            let crate_requirements = crate_requirements
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>();

            log::error!(
                "Crate `{}@{}` is within the cooldown period.\n\t\
No versions older than {} minutes satisfy semver constraints {crate_requirements:?}.\n\t\
Relax the constraints, wait for the cooldown to elapse, or allowlist this crate.\n",
                failure.name,
                failure.current_version,
                failure.minimum_minutes,
            );
        } else {
            let versions = version_candidates
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>();

            log::error!(
                "Crate `{}@{}` fails the cooldown period. \
To resolve this, downgrade to one of these versions: {versions:?} by running\n\t\
`cargo update {} --precise <version>`\n",
                failure.name,
                failure.current_version,
                failure.name
            );
        }
    }

    Ok(())
}

fn detect_cooldown_failure(candidate: CooldownCandidate<'_>) -> Option<CooldownFailure> {
    if candidate.age_minutes < candidate.minimum_minutes {
        log::debug!(
            "Crate fails cooldown period: crate = {}@{}, age_minutes = {}, minimum_minutes = {}",
            candidate.package.name,
            candidate.package.version,
            candidate.age_minutes,
            candidate.minimum_minutes
        );
        Some(candidate.into())
    } else {
        None
    }
}

fn gather_dependencies_requirements(
    crate_names: Vec<String>,
    workspace: &Workspace,
) -> HashMap<PackageId, HashSet<VersionReq>> {
    let mut version_requirements: HashMap<PackageId, HashSet<VersionReq>> = HashMap::new();
    let packages = workspace.packages();

    let dependencies_by_package_id = workspace.nodes.iter().flat_map(|node| {
        let pkg = packages
            .get(&node.id)
            .unwrap_or_else(|| panic!("Could not find associated package to {:?}", node.id));

        let pkg_dependencies: &[Dependency] = pkg.dependencies.as_ref();

        node.dependencies.iter().filter_map(|dep| {
            // Find the dependency matching the current node, if it's in the `crate_names`
            // list.
            let dependency = pkg_dependencies.iter().find(|dependency| {
                packages.get(dep).is_some_and(|package| {
                    package.name.as_str() == dependency.name
                        && crate_names.contains(&dependency.name)
                })
            });
            dependency.map(|dependency| (dep.clone(), dependency))
        })
    });
    for (package, dependency) in dependencies_by_package_id {
        let requirements = version_requirements.entry(package.clone()).or_default();
        requirements.insert(dependency.req.clone());
    }
    version_requirements
}

fn ensure_lockfile(workspace: &Workspace) -> Result<()> {
    let mut workspace_root = workspace.root_path();
    workspace_root.push("Cargo.lock");
    if workspace_root.exists() {
        return Ok(());
    }
    bail!("`Cargo.lock` file does not exist");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allowlist::{AllowExact, AllowPackage, AllowSection};

    const SEVEN_DAYS_MINUTES: u64 = 10_080;

    fn local_package() -> Package {
        let workspace = Workspace::load().unwrap();
        workspace
            .packages()
            .into_values()
            .find(|p| p.source.is_none())
            .unwrap()
    }

    fn registry_package() -> Package {
        let workspace = Workspace::load().unwrap();
        workspace
            .packages()
            .into_values()
            .find(|p| p.source.is_some())
            .unwrap()
    }

    #[test]
    fn gather_dependencies_requirements_only_includes_requested_crate_names() {
        let workspace = Workspace::load().unwrap();
        let packages = workspace.packages();

        let result = gather_dependencies_requirements(vec!["tokio".to_string()], &workspace);
        assert!(!result.is_empty(), "expected at least one entry for tokio");

        for pkg_id in result.keys() {
            let package = packages
                .get(pkg_id)
                .unwrap_or_else(|| panic!("PackageId {pkg_id:?} not found in packages"));
            assert_eq!(
                package.name.as_str(),
                "tokio",
                "expected all keys to correspond to tokio, but found {:?}",
                package.name
            );
        }

        let result = gather_dependencies_requirements(
            vec!["nonexistent_crate_xyz_123".to_string()],
            &workspace,
        );
        assert!(
            result.is_empty(),
            "expected empty map for nonexistent crate, got {} entries",
            result.len()
        );
    }

    #[test]
    fn gather_dependencies_requirements_consolidates_from_multiple_dependents() {
        let workspace = Workspace::load().unwrap();

        let dependencies_requirements =
            gather_dependencies_requirements(vec!["tokio".to_string()], &workspace);
        assert_eq!(
            dependencies_requirements.len(),
            1,
            "expected exactly one PackageId for tokio, got {}",
            dependencies_requirements.len()
        );

        let requirements = dependencies_requirements.values().next().unwrap();
        assert!(
            requirements.len() > 1,
            "expected multiple distinct VersionReqs for tokio, got {}",
            requirements.len()
        );

        let version_requirements: Vec<String> = requirements
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        assert!(version_requirements.contains(&"^1".to_string()),);
        assert!(version_requirements.contains(&"^1.21.2".to_string()),);
    }

    #[test]
    fn detect_cooldown_failure_returns_failure_when_too_young() {
        let package = registry_package();

        let candidate = CooldownCandidate {
            package: &package,
            age_minutes: 5,
            minimum_minutes: SEVEN_DAYS_MINUTES,
        };
        let result = detect_cooldown_failure(candidate);
        let failure = result.expect("expected a cooldown failure for a just-published version");
        assert_eq!(failure.name, package.name.as_str());
        assert_eq!(failure.current_version, package.version.to_string());
        assert_eq!(failure.minimum_minutes, SEVEN_DAYS_MINUTES);
    }

    #[test]
    fn detect_cooldown_failure_returns_none_when_old_enough() {
        let package = registry_package();

        let candidate = CooldownCandidate {
            package: &package,
            age_minutes: SEVEN_DAYS_MINUTES * 2,
            minimum_minutes: SEVEN_DAYS_MINUTES,
        };
        let result = detect_cooldown_failure(candidate);
        assert!(
            result.is_none(),
            "expected no failure for a 2-weeks-old version"
        );
    }

    #[test]
    fn cooldown_requirement_returns_none_for_local_dependency() {
        let package = local_package();
        let config = Config::default();
        let allowlist = Allowlist::default();

        let result = cooldown_requirement(&package, &config, &allowlist);
        assert!(result.is_none(), "local dependencies should be skipped");
    }

    #[test]
    fn cooldown_requirement_returns_none_for_non_allowed_registry() {
        let package = registry_package();
        let config = Config {
            allowed_registries: vec![],
            ..Config::default()
        };
        let allowlist = Allowlist::default();

        let result = cooldown_requirement(&package, &config, &allowlist);
        assert!(
            result.is_none(),
            "packages from non-allowed registries should be skipped"
        );
    }

    #[test]
    fn cooldown_requirement_returns_some_with_global_cooldown() {
        let package = registry_package();
        let config = Config::default();
        let allowlist = Allowlist::default();

        let result = cooldown_requirement(&package, &config, &allowlist);
        let (returned_pkg, minutes) = result.expect("registry package should require cooldown");
        assert_eq!(returned_pkg.id, package.id);
        assert_eq!(minutes, config.cooldown_minutes);
    }

    #[test]
    fn cooldown_requirement_uses_per_crate_override_when_lower() {
        let package = registry_package();
        let config = Config::default();
        let allowlist = Allowlist {
            allow: AllowSection {
                package: vec![AllowPackage {
                    crate_name: package.name.to_string(),
                    minutes: Some(60),
                }],
                ..AllowSection::default()
            },
        };

        let (_, minutes) = cooldown_requirement(&package, &config, &allowlist).unwrap();
        assert_eq!(minutes, 60);
    }

    #[test]
    fn cooldown_requirement_uses_global_when_lower_than_per_crate() {
        let package = registry_package();
        let config = Config {
            cooldown_minutes: 100,
            ..Config::default()
        };
        let allowlist = Allowlist {
            allow: AllowSection {
                package: vec![AllowPackage {
                    crate_name: package.name.to_string(),
                    minutes: Some(50_000),
                }],
                ..AllowSection::default()
            },
        };

        let (_, minutes) = cooldown_requirement(&package, &config, &allowlist).unwrap();
        assert_eq!(minutes, 100);
    }

    #[test]
    fn cooldown_requirement_returns_none_when_per_crate_minutes_is_zero() {
        let package = registry_package();
        let config = Config::default();
        let allowlist = Allowlist {
            allow: AllowSection {
                package: vec![AllowPackage {
                    crate_name: package.name.to_string(),
                    minutes: Some(0),
                }],
                ..AllowSection::default()
            },
        };

        let result = cooldown_requirement(&package, &config, &allowlist);
        assert!(
            result.is_none(),
            "zero per-crate cooldown should exempt the package"
        );
    }

    #[test]
    fn cooldown_requirement_returns_none_for_exact_allowlisted_version() {
        let package = registry_package();
        let config = Config::default();
        let allowlist = Allowlist {
            allow: AllowSection {
                exact: vec![AllowExact {
                    crate_name: package.name.to_string(),
                    version: package.version.to_string(),
                }],
                ..AllowSection::default()
            },
        };

        let result = cooldown_requirement(&package, &config, &allowlist);
        assert!(
            result.is_none(),
            "exact-allowlisted version should be skipped"
        );
    }
}
