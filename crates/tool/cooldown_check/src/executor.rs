use std::{
    collections::{HashMap, HashSet},
    result::Result::Ok,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use cargo_metadata::{Dependency, PackageId};
use semver::{Version, VersionReq};

use crate::{
    cache::Cache,
    registry::{RegistryClient, VersionMeta},
    resolver::{age_minutes, filter_candidates},
    workspace::Workspace,
};

pub async fn run_check_flow(workspace: Workspace) -> Result<()> {
    ensure_lockfile(&workspace)?;

    let allowlist = &workspace.allowlist;
    let config = &workspace.config;
    let cache = if let Some(ref root) = config.cache_dir {
        Cache::with_root(root.clone(), Duration::from_secs(config.ttl_seconds))?
    } else {
        Cache::new(config.ttl_seconds)?
    };
    let client = RegistryClient::new(config)?;

    let packages = workspace.packages();

    let mut offending_crates: Vec<OffenderCrate> = Vec::new();

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

        let meta = fetch_version_meta(&client, &cache, pkg.name.as_str(), &current_version).await?;
        let age_minutes = age_minutes(meta.created_at);
        if age_minutes < minimum_minutes as i64 {
            log::debug!("crate offends cooldown period: crate = {}@{}, age_minutes = {age_minutes}, minimum_minutes = {minimum_minutes}, created_at = {}", pkg.name, pkg.version, meta.created_at);
            offending_crates.push(OffenderCrate {
                package_id: node.id.clone(),
                name: pkg.name.to_string(),
                current_version: current_version.clone(),
                minimum_minutes,
            });
        }
    }

    if offending_crates.is_empty() {
        log::info!("dependency graph passed cooldown check ✅");
        Ok(())
    } else {
        let offending_crates = offending_crates.into_iter().collect::<HashSet<_>>();
        log_offending_crates(&workspace, &client, &cache, offending_crates).await?;
        bail!("dependency graph contains crates within the cooldown period ❌")
    }
}

async fn log_offending_crates(
    workspace: &Workspace,
    client: &RegistryClient,
    cache: &Cache,
    offending_crates: HashSet<OffenderCrate>,
) -> anyhow::Result<()> {
    let offending_crate_names = offending_crates
        .iter()
        .map(|offending_crate| offending_crate.name.clone())
        .collect::<Vec<_>>();

    let version_requirements = gather_dependencies_requirements(offending_crate_names, workspace);

    for offender_crate in offending_crates {
        let crate_requirements = version_requirements
            .get(&offender_crate.package_id)
            .cloned()
            .unwrap_or_default();
        let version_candidates =
            crate_version_candidates(client, cache, &offender_crate, &crate_requirements)
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
                offender_crate.name,
                offender_crate.current_version,
                offender_crate.minimum_minutes,
            );
        } else {
            let versions = version_candidates
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>();

            log::error!(
                "crate `{}@{}` offends the cooldown period. \
To resolve this, downgrade to one of these versions: {versions:?} by running\n\t\
`cargo update {} --precise <version>`\n",
                offender_crate.name,
                offender_crate.current_version,
                offender_crate.name
            );
        }
    }

    Ok(())
}

async fn crate_version_candidates(
    client: &RegistryClient,
    cache: &Cache,
    offender_crate: &OffenderCrate,
    requirements: &[VersionReq],
) -> anyhow::Result<Vec<Version>> {
    let current_version = Version::parse(&offender_crate.current_version).context(format!(
        "Could not parse {}@{} version",
        offender_crate.name, offender_crate.current_version
    ))?;
    let candidate_list = fetch_version_list(client, cache, &offender_crate.name).await?;
    let versions = filter_candidates(candidate_list, offender_crate.minimum_minutes);
    let versions = versions
        .into_iter()
        .filter_map(|meta| Version::parse(&meta.num).ok())
        .filter(|version| {
            *version < current_version && satisfies_requirements(version, requirements)
        })
        .collect::<Vec<_>>();

    Ok(versions)
}

fn gather_dependencies_requirements(
    crate_names: Vec<String>,
    workspace: &Workspace,
) -> HashMap<PackageId, Vec<VersionReq>> {
    let mut version_requirements: HashMap<PackageId, Vec<VersionReq>> = HashMap::new();
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
    for (pkg, dep) in dependencies_by_package_id {
        let requirements = version_requirements.entry(pkg.clone()).or_default();
        if !requirements.iter().any(|req| req == &dep.req) {
            requirements.push(dep.req.clone());
        }
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct OffenderCrate {
    package_id: PackageId,
    name: String,
    current_version: String,
    minimum_minutes: u64,
}

async fn fetch_version_meta(
    client: &RegistryClient,
    cache: &Cache,
    name: &str,
    version: &str,
) -> Result<VersionMeta> {
    let key = format!("{name}/{version}");
    if let Some(meta) = cache.get::<VersionMeta>(&key)? {
        return Ok(meta);
    }
    let meta = client.fetch_version(name, version).await?;
    cache.put(&key, &meta)?;
    Ok(meta)
}

async fn fetch_version_list(
    client: &RegistryClient,
    cache: &Cache,
    name: &str,
) -> Result<Vec<VersionMeta>> {
    let key = format!("{name}/_list");
    if let Some(list) = cache.get::<Vec<VersionMeta>>(&key)? {
        return Ok(list);
    }
    let mut versions = client.list_versions(name).await?;
    versions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    cache.put(&key, &versions)?;
    Ok(versions)
}

fn satisfies_requirements(version: &Version, requirements: &[VersionReq]) -> bool {
    if requirements.is_empty() {
        return true;
    }
    log::debug!(
        "Analyzing version `{version}` against requirements {:?}",
        requirements
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
    );
    requirements.iter().all(|req| req.matches(version))
}
