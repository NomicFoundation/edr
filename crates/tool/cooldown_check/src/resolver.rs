use std::{sync::LazyLock, time::Duration};

use anyhow::Context;
use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};

use crate::{
    cache::Cache,
    config::Config,
    cooldown_failure::CooldownFailure,
    registry::{RegistryClient, VersionMeta},
};

// A single reference point in time for all cooldown comparisons,
/// ensuring consistency across the entire check.
static NOW: LazyLock<DateTime<Utc>> = LazyLock::new(Utc::now);

pub fn age_minutes(meta: &VersionMeta) -> i64 {
    (*NOW - meta.created_at).num_minutes()
}

pub struct Resolver {
    client: RegistryClient,
    cache: Cache,
}

impl Resolver {
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let cache = if let Some(ref root) = config.cache_dir {
            Cache::with_root(root.clone(), Duration::from_secs(config.ttl_seconds))?
        } else {
            Cache::new(config.ttl_seconds)?
        };
        let client = RegistryClient::new(config)?;

        Ok(Self { client, cache })
    }

    pub async fn crate_version_candidates(
        &self,
        cooldown_failure: &CooldownFailure,
        requirements: &[VersionReq],
    ) -> anyhow::Result<Vec<Version>> {
        let current_version =
            Version::parse(&cooldown_failure.current_version).context(format!(
                "Could not parse {}@{} version",
                cooldown_failure.name, cooldown_failure.current_version
            ))?;
        let candidate_list = self.fetch_version_list(&cooldown_failure.name).await?;
        let versions =
            filter_candidates_by_time(candidate_list, cooldown_failure.minimum_minutes, *NOW);
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
        &self,
        name: &str,
        version: &str,
    ) -> anyhow::Result<VersionMeta> {
        let key = format!("{name}/{version}");
        if let Some(meta) = self.cache.get::<VersionMeta>(&key)? {
            return Ok(meta);
        }
        let meta = self.client.fetch_version(name, version).await?;
        self.cache.put(&key, &meta)?;
        Ok(meta)
    }

    async fn fetch_version_list(&self, name: &str) -> anyhow::Result<Vec<VersionMeta>> {
        let key = format!("{name}/_list");
        if let Some(list) = self.cache.get::<Vec<VersionMeta>>(&key)? {
            return Ok(list);
        }
        let mut versions = self.client.list_versions(name).await?;
        versions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        self.cache.put(&key, &versions)?;
        Ok(versions)
    }
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
