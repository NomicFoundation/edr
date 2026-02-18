use std::{
    collections::{HashMap, HashSet},
    result::Result::Ok,
    time::Duration,
};

use anyhow::{bail, Result};
use cargo_metadata::PackageId;
use semver::{Version, VersionReq};

use crate::{
    cache::Cache,
    registry::{RegistryClient, VersionMeta},
    resolver::{age_minutes, filter_candidates, Candidate},
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
        log::info!("dependency graph is cool ✅");
        Ok(())
    } else {
        identify_offending_crates(&workspace, &client, &cache, offending_crates).await?;
        bail!("dependency graph offends cooldown period ❌")
    }
}

async fn identify_offending_crates(
    workspace: &Workspace,
    client: &RegistryClient,
    cache: &Cache,
    offending_crates: Vec<OffenderCrate>,
) -> anyhow::Result<()> {
    let offending_crate_names = offending_crates
        .iter()
        .map(|offending_crate| offending_crate.name.clone())
        .collect::<Vec<_>>();
    let mut visited_failures: HashSet<String> = HashSet::new();

    let version_requirements = gather_dependencies_requirements(offending_crate_names, workspace);

    for offender_crate in offending_crates {
        let key = format!("{}@{}", offender_crate.name, offender_crate.current_version);
        if visited_failures.contains(&key) {
            // a warning was already emmitted for this dependency
            continue;
        }

        let crate_requirements = version_requirements
            .get(&offender_crate.package_id)
            .cloned()
            .unwrap_or_default();
        let candidates =
            crate_version_candidates(client, cache, &offender_crate, &crate_requirements).await?;
        if candidates.is_empty() {
            visited_failures.insert(key.clone());
            log::error!(
                    "crate `{}` has no versions older than {} minutes that satisfy the current semver constraints {:?}.\n\tRelax these constraints, wait for the cooldown period, or add this crate to the allowlist configuration if this version is needed for security improvements.\n",
                    offender_crate.name,
                    offender_crate.minimum_minutes,
                    crate_requirements.iter().map(std::string::ToString::to_string).collect::<Vec<_>>(),
                );
            continue;
        }

        log::error!("crate `{}@{}` offends the cooldown period. To resolve this, downgrade to one of these versions: {:?} by running\n\t`cargo update {} --precise <version>`\n", offender_crate.name, offender_crate.current_version, candidates.into_iter().map(|candidate| candidate.version).collect::<Vec<String>>(), offender_crate.name);
    }

    Ok(())
}

async fn crate_version_candidates(
    client: &RegistryClient,
    cache: &Cache,
    offender_crate: &OffenderCrate,
    requirements: &[VersionReq],
) -> anyhow::Result<Vec<Candidate>> {
    let candidate_list = fetch_version_list(client, cache, &offender_crate.name).await?;

    let mut candidates = filter_candidates(candidate_list, offender_crate.minimum_minutes);

    candidates.retain(|candidate| satisfies_requirements(&candidate.version, requirements));

    if let Ok(current_semver) = Version::parse(&offender_crate.current_version) {
        candidates.retain(|candidate| {
            Version::parse(&candidate.version)
                .map(|version| version < current_semver)
                .unwrap_or(true)
        });
    }
    Ok(candidates)
}

fn gather_dependencies_requirements(
    crate_names: Vec<String>,
    workspace: &Workspace,
) -> HashMap<PackageId, Vec<VersionReq>> {
    let mut version_requirements: HashMap<PackageId, Vec<VersionReq>> = HashMap::new();
    let packages = workspace.packages();

    for node in &workspace.nodes {
        let pkg = packages
            .get(&node.id)
            .unwrap_or_else(|| panic!("Could not find associated package to {:?}", node.id));
        for dep in node
            .deps
            .iter()
            .filter(|dep| crate_names.contains(&dep.name))
        {
            let dep_pkg = packages
                .get(&dep.pkg)
                .unwrap_or_else(|| panic!("Could not find associated package to {:?}", dep.pkg));
            if let Some(manifest_dep) =
                find_manifest_dependency(&pkg.dependencies, &dep.name, &dep_pkg.name)
            {
                let requirements = version_requirements.entry(dep.pkg.clone()).or_default();
                if !requirements.iter().any(|req| req == &manifest_dep.req) {
                    requirements.push(manifest_dep.req.clone());
                }
            }
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

#[derive(Clone, Debug)]
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
    let list = client.list_versions(name).await?;
    cache.put(&key, &list)?;
    Ok(list)
}

fn find_manifest_dependency<'a>(
    deps: &'a [cargo_metadata::Dependency],
    dep_name: &str,
    package_name: &str,
) -> Option<&'a cargo_metadata::Dependency> {
    deps.iter().find(|candidate| {
        candidate
            .rename
            .as_deref()
            .is_some_and(|rename| rename == dep_name)
            || candidate.name == dep_name
            || candidate.name == package_name
    })
}

fn satisfies_requirements(version: &str, requirements: &[VersionReq]) -> bool {
    if requirements.is_empty() {
        return true;
    }
    match Version::parse(version) {
        Ok(parsed) => {
            log::debug!(
                "Analyzing version `{parsed}` against requirements {:?}",
                requirements
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
            );
            requirements.iter().all(|req| req.matches(&parsed))
        }
        Err(_) => false,
    }
}
