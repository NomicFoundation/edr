use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::Path,
    process::Command,
    result::Result::Ok,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use cargo_metadata::PackageId;
use clap_cargo::{Features, Manifest};
use semver::{Op, Version, VersionReq};

use crate::{
    cache::Cache,
    config::Config,
    metadata::read_metadata,
    registry::{RegistryClient, VersionMeta},
    resolver::{age_minutes, filter_candidates},
};

pub async fn run_check_flow(
    config: &Config,
    manifest: &Manifest,
    features: &Features,
) -> Result<()> {
    ensure_lockfile()?;

    let allowlist = &config.allowlist;
    let per_crate_minutes = allowlist.per_crate_minutes();
    let cache = if let Some(ref root) = config.cache_dir {
        Cache::with_root(root.clone(), Duration::from_secs(config.ttl_seconds))?
    } else {
        Cache::new(config.ttl_seconds)?
    };
    let client = RegistryClient::new(config)?;

    let metadata = read_metadata(manifest, features)?;

    let resolve = metadata
        .resolve
        .clone()
        .context("cargo metadata output did not include a resolved dependency graph")?;
    let packages: HashMap<PackageId, cargo_metadata::Package> = metadata
        .packages
        .into_iter()
        .map(|pkg| (pkg.id.clone(), pkg))
        .collect();

    let mut fresh_entries: Vec<FreshCrate> = Vec::new();
    let mut equality_dependents: HashMap<PackageId, Vec<PackageId>> = HashMap::new();
    let mut version_requirements: HashMap<PackageId, Vec<VersionReq>> = HashMap::new();
    let mut seen: HashSet<PackageId> = HashSet::new();

    let cooldown_minutes = allowlist
        .global_minutes()
        .map_or(config.cooldown_minutes, |global| {
            config.cooldown_minutes.min(global)
        });

    for node in &resolve.nodes {
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
        let minimum_minutes = per_crate_minutes
            .get(pkg.name.as_str())
            .map_or(cooldown_minutes, |minutes| cooldown_minutes.min(*minutes));

        let exact_allowed = allowlist.is_exact_allowed(pkg.name.as_str(), &current_version);

        // +++++++++ CHECK ⬇️ ++++++++++++++++
        for dep in &node.deps {
            let dep_pkg = packages
                .get(&dep.pkg)
                .unwrap_or_else(|| panic!("Could not find associated package to {:?}", dep.pkg));
            if dep_pkg
                .source
                .as_ref()
                .is_some_and(|source| !config.is_registry_allowed(&source.repr))
            {
                log::warn!(
                    "skipping non-crates.io registry dependency. crate = {}, source = {}",
                    pkg.name,
                    dep_pkg
                        .source
                        .as_ref()
                        .map(|source| &source.repr)
                        .expect("Source should be present")
                );
                continue;
            }

            if let Some(manifest_dep) =
                find_manifest_dependency(&pkg.dependencies, &dep.name, &dep_pkg.name)
            {
                let requirements = version_requirements.entry(dep.pkg.clone()).or_default();
                if !requirements.iter().any(|req| req == &manifest_dep.req) {
                    requirements.push(manifest_dep.req.clone());
                }

                if is_exact_requirement(&manifest_dep.req) {
                    equality_dependents
                        .entry(dep.pkg.clone())
                        .or_default()
                        .push(node.id.clone());
                }
            }
        }

        if exact_allowed || minimum_minutes == 0 {
            continue;
        }

        if pkg.source.is_some() {
            // no need to check version meta for local dependencies (workspace crates)
            match fetch_version_meta(&client, &cache, pkg.name.as_str(), &current_version).await {
                Ok(meta) => {
                    let age_minutes = age_minutes(meta.created_at);
                    log::trace!(
                        "crate age inspected. crate = {}, age_minutes = {age_minutes}, minimum_minutes = {minimum_minutes}, creted_at = {}", pkg.name, meta.created_at
                    );
                    if age_minutes < minimum_minutes as i64 {
                        fresh_entries.push(FreshCrate {
                            package_id: node.id.clone(),
                            name: pkg.name.to_string(),
                            current_version: current_version.clone(),
                            minimum_minutes,
                        });
                    }
                }
                Err(err) => {
                    if config.offline_ok {
                        log::warn!(
                        "skipping metadata fetch due to offline mode. crate = {}, error = {err}",
                        pkg.name
                    );
                    } else {
                        return Err(err);
                    }
                }
            }
        }
    }

    if fresh_entries.is_empty() {
        log::info!("dependency graph is cool ✅");
        Ok(())
    } else {
        identify_violating_entries(
            &client,
            &cache,
            fresh_entries,
            equality_dependents,
            version_requirements,
            config.offline_ok,
        )
        .await?;
        bail!("dependency graph violates cooldown period ❌")
    }
}

#[allow(clippy::too_many_arguments)]
async fn identify_violating_entries(
    client: &RegistryClient,
    cache: &Cache,
    mut fresh_entries: Vec<FreshCrate>,
    equality_dependents: HashMap<PackageId, Vec<PackageId>>,
    version_requirements: HashMap<PackageId, Vec<VersionReq>>,
    offline_ok: bool,
) -> anyhow::Result<()> {
    let mut visited_failures: HashSet<String> = HashSet::new();

    let fresh_ids: HashSet<PackageId> =
        fresh_entries.iter().map(|f| f.package_id.clone()).collect();

    fresh_entries.sort_by_key(|entry| {
        equality_dependents
            .get(&entry.package_id)
            .map_or(0, |dependents| {
                dependents
                    .iter()
                    .filter(|id| fresh_ids.contains(*id))
                    .count()
            })
    });

    let mut queue: VecDeque<FreshCrate> = fresh_entries.into();

    while let Some(fresh) = queue.pop_front() {
        let key = format!("{}@{}", fresh.name, fresh.current_version);
        if visited_failures.contains(&key) {
            // a warning was already emmitted for this dependency
            continue;
        }

        let candidate_list = match fetch_version_list(client, cache, &fresh.name).await {
            Ok(list) => list,
            Err(err) => {
                if offline_ok {
                    log::warn!("skipping candidate discovery due to offline mode. crate = {}, error = {err}", fresh.name);
                    queue.push_back(fresh);
                    continue;
                } else {
                    bail!(err);
                }
            }
        };

        let mut candidates = filter_candidates(candidate_list, fresh.minimum_minutes);
        let requirements = version_requirements
            .get(&fresh.package_id)
            .cloned()
            .unwrap_or_default();
        if !requirements.is_empty() {
            candidates
                .retain(|candidate| satisfies_requirements(&candidate.version, &requirements));
        }

        if let Ok(current_semver) = Version::parse(&fresh.current_version) {
            candidates.retain(|candidate| {
                Version::parse(&candidate.version)
                    .map(|version| version < current_semver)
                    .unwrap_or(true)
            });
        }

        if candidates.is_empty() {
            visited_failures.insert(key.clone());
            log::error!(
                    "crate `{}` has no versions older than {} minutes that satisfy the current semver constraints {:?}.\n\tRelax these constraints, wait for the cooldown period, or add this crate to the allowlist configuration if this version is needed for security improvements.\n",
                    fresh.name,
                    fresh.minimum_minutes,
                    requirements.iter().map(std::string::ToString::to_string).collect::<Vec<_>>(),
                );
            continue;
        }

        log::error!("crate `{}@{}` violates the cooldown period. To resolve this, downgrade to one of these versions: {:?} by running\n\t`cargo update {} --precise <version>`\n", fresh.name, fresh.current_version, candidates.into_iter().map(|candidate| candidate.version).collect::<Vec<String>>(), fresh.name);
    }

    Ok(())
}

fn ensure_lockfile() -> Result<()> {
    if Path::new("Cargo.lock").exists() {
        return Ok(());
    }
    let status = Command::new("cargo").args(["generate-lockfile"]).status()?;
    if !status.success() {
        bail!("failed to generate Cargo.lock via `cargo generate-lockfile`");
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct FreshCrate {
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

fn is_exact_requirement(req: &semver::VersionReq) -> bool {
    if req.comparators.len() != 1 {
        return false;
    }
    matches!(req.comparators.first().map(|comp| comp.op), Some(Op::Exact))
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
