use std::sync::LazyLock;

use anyhow::Context;
use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};

use crate::{
    cache::Cache,
    offender::OffenderCrate,
    registry::{RegistryClient, VersionMeta},
};

// Create the static singleton instance
static NOW: LazyLock<DateTime<Utc>> = LazyLock::new(|| {
    // This closure runs only once, the first time NOW is accessed
    Utc::now()
});

pub fn age_minutes(datetime: DateTime<Utc>) -> i64 {
    (*NOW - datetime).num_minutes()
}

pub async fn crate_version_candidates(
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
    let versions = filter_candidates_by_time(candidate_list, offender_crate.minimum_minutes, *NOW);
    let versions = versions
        .into_iter()
        .filter_map(|meta| Version::parse(&meta.num).ok())
        .filter(|version| {
            *version < current_version && satisfies_requirements(version, requirements)
        })
        .collect::<Vec<_>>();

    Ok(versions)
}

pub async fn fetch_version_meta(
    client: &RegistryClient,
    cache: &Cache,
    name: &str,
    version: &str,
) -> anyhow::Result<VersionMeta> {
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
) -> anyhow::Result<Vec<VersionMeta>> {
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

pub fn filter_candidates_by_time(
    versions: Vec<VersionMeta>,
    minimum_minutes: u64,
    now: DateTime<Utc>,
) -> Vec<VersionMeta> {
    let cutoff = now - chrono::Duration::minutes(minimum_minutes as i64);
    versions
        .into_iter()
        .filter(|meta| !meta.yanked)
        .filter(|meta| meta.created_at <= cutoff)
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn filters_fresh_versions() {
        let now = Utc.with_ymd_and_hms(2024, 10, 1, 0, 0, 0).unwrap();
        let versions = vec![
            VersionMeta {
                created_at: Utc.with_ymd_and_hms(2024, 9, 30, 23, 50, 0).unwrap(),
                yanked: false,
                num: "1.2.3".into(),
            },
            VersionMeta {
                created_at: Utc.with_ymd_and_hms(2024, 9, 30, 22, 0, 0).unwrap(),
                yanked: false,
                num: "1.2.2".into(),
            },
            VersionMeta {
                created_at: Utc.with_ymd_and_hms(2024, 9, 30, 20, 0, 0).unwrap(),
                yanked: true,
                num: "1.2.1".into(),
            },
        ];
        let candidates = filter_candidates_by_time(versions, 30, now);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].num, "1.2.2");
    }
}
