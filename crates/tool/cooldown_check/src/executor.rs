use std::{
    collections::{HashMap, HashSet},
    result::Result::Ok,
};

use anyhow::{bail, Result};
use cargo_metadata::{Dependency, PackageId};
use semver::VersionReq;

use crate::{
    cooldown_failure::CooldownFailure,
    resolver::{age_minutes, Resolver},
    workspace::Workspace,
};

pub async fn run_check_flow(workspace: Workspace) -> Result<()> {
    ensure_lockfile(&workspace)?;

    let allowlist = &workspace.allowlist;
    let config = &workspace.config;
    let packages = workspace.packages();
    let resolver = Resolver::new(config)?;

    let mut cooldown_failures: Vec<CooldownFailure> = Vec::new();

    let mut seen: HashSet<PackageId> = HashSet::new();

    let cooldown_minutes = config.cooldown_minutes;

    if cooldown_minutes == 0 {
        log::info!("skipping cooldown check: cooldown minutes is set to 0");
    }

    for node in &workspace.nodes {
        if !seen.insert(node.id.clone()) {
            continue;
        }
        let pkg = packages
            .get(&node.id)
            .unwrap_or_else(|| panic!("Could not find associated package to {:?}", node.id));
        if pkg
            .source
            .as_ref()
            .is_some_and(|source| !config.is_registry_allowed(&source.repr))
        {
            log::warn!(
                "skipping non-crates.io registry dependency. crate = {}, source = {}",
                pkg.name,
                pkg.source
                    .as_ref()
                    .map(|source| &source.repr)
                    .expect("Source should be present")
            );
            continue;
        }

        let current_version = pkg.version.to_string();
        let minimum_minutes = allowlist
            .per_crate_minutes()
            .get(pkg.name.as_str())
            .map_or(cooldown_minutes, |minutes| cooldown_minutes.min(*minutes));
        let exact_allowed = allowlist.is_exact_allowed(pkg.name.as_str(), &current_version);
        let is_local_dependency = pkg.source.is_none();

        if is_local_dependency {
            log::debug!(
                "skipping validation for crate {}@{}: crate is a local dependency",
                pkg.name,
                pkg.version
            );
            continue;
        }
        if minimum_minutes == 0 {
            log::info!("skipping validation for crate {}@{}: `allow.package.minutes` is set to 0 in the allowlist", pkg.name, pkg.version);
            continue;
        }
        if exact_allowed {
            log::info!(
                "skipping validation for crate {}@{}: version is listed as `allow.exact`",
                pkg.name,
                pkg.version
            );
            continue;
        }

        let meta = resolver
            .fetch_version_meta(pkg.name.as_str(), &current_version)
            .await?;
        let age_minutes = age_minutes(&meta);
        if age_minutes < minimum_minutes as i64 {
            log::debug!("crate fails cooldown period: crate = {}@{}, age_minutes = {age_minutes}, minimum_minutes = {minimum_minutes}, created_at = {}", pkg.name, pkg.version, meta.created_at);
            cooldown_failures.push(CooldownFailure {
                package_id: node.id.clone(),
                name: pkg.name.to_string(),
                current_version: current_version.clone(),
                minimum_minutes,
            });
        }
    }

    if cooldown_failures.is_empty() {
        log::info!("dependency graph passed cooldown check ✅");
        Ok(())
    } else {
        let failures = cooldown_failures.into_iter().collect::<HashSet<_>>();
        report_cooldown_failures(&workspace, &resolver, failures).await?;
        bail!("dependency graph failed cooldown check ❌")
    }
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
                "crate `{}@{}` is within the cooldown period.\n\t\
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
                "crate `{}@{}` fails the cooldown period. \
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

        let pkg_dependencies: &Vec<Dependency> = pkg.dependencies.as_ref();

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

    #[test]
    fn only_includes_requested_crate_names() {
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
    fn consolidates_requirements_from_multiple_dependents() {
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
}
