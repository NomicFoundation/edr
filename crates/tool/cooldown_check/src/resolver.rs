use std::{sync::LazyLock, time::Duration};

use anyhow::Context;
use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};

use crate::{
    cache::Cache,
    config::Config,
    registry::{RegistryClient, VersionMeta},
    types::CooldownFailure,
};

/// A single reference point in time for all cooldown comparisons,
/// ensuring consistency across the entire check.
static NOW: LazyLock<DateTime<Utc>> = LazyLock::new(Utc::now);

fn age_minutes(meta: &VersionMeta) -> u64 {
    (*NOW - meta.created_at)
        .num_minutes()
        .try_into()
        .unwrap_or(0)
}

pub struct Resolver {
    client: RegistryClient,
    cache: Cache,
}

impl Resolver {
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let cache = if let Some(ref root) = config.cache_dir {
            Cache::with_root(root.clone(), Duration::from_secs(config.cache_ttl_seconds))?
        } else {
            Cache::new(Duration::from_secs(config.cache_ttl_seconds))?
        };
        let client = RegistryClient::new(config)?;

        Ok(Self { client, cache })
    }

    pub async fn find_version_candidates(
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
        let cutoff = *NOW - chrono::Duration::minutes(cooldown_failure.minimum_minutes as i64);

        let versions = candidate_list
            .into_iter()
            .filter(|meta| !meta.yanked)
            .filter(|meta| meta.created_at <= cutoff)
            .filter_map(|meta| Version::parse(&meta.num).ok())
            .filter(|version| {
                *version < current_version && satisfies_requirements(version, requirements)
            })
            .collect::<Vec<_>>();

        Ok(versions)
    }

    pub async fn fetch_version_age(&self, name: &str, version: &str) -> anyhow::Result<u64> {
        let key = format!("{name}/{version}");
        if let Some(meta) = self.cache.get::<VersionMeta>(&key)? {
            return Ok(age_minutes(&meta));
        }
        let meta = self.client.fetch_version(name, version).await?;
        self.cache.put(&key, &meta)?;
        Ok(age_minutes(&meta))
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
    requirements.iter().all(|req| req.matches(version))
}

#[cfg(test)]
mod tests {
    use cargo_metadata::PackageId;
    use chrono::TimeZone;
    use tempfile::tempdir;

    use super::*;

    fn tokio_versions() -> Vec<VersionMeta> {
        let old = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        vec![
            VersionMeta {
                num: "1.44.0".into(),
                created_at: old,
                yanked: false,
            },
            VersionMeta {
                num: "1.43.4".into(),
                created_at: old,
                yanked: false,
            },
            VersionMeta {
                num: "1.43.3".into(),
                created_at: old,
                yanked: false,
            },
            VersionMeta {
                num: "1.43.0".into(),
                created_at: old,
                yanked: false,
            },
            VersionMeta {
                num: "1.42.1".into(),
                created_at: old,
                yanked: false,
            },
            VersionMeta {
                num: "1.41.0".into(),
                created_at: old,
                yanked: false,
            },
        ]
    }

    fn tokio_failure() -> CooldownFailure {
        CooldownFailure {
            package_id: PackageId {
                repr: "tokio 1.43.4".to_string(),
            },
            name: "tokio".to_string(),
            current_version: "1.43.4".to_string(),
            minimum_minutes: 0,
        }
    }

    fn resolver_with_cached_tokio_versions() -> (Resolver, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let config = Config {
            cache_dir: Some(dir.path().to_path_buf()),
            ..Config::default()
        };
        let resolver = Resolver::new(&config).unwrap();
        resolver
            .cache
            .put("tokio/_list", &tokio_versions())
            .unwrap();
        (resolver, dir)
    }

    #[tokio::test]
    async fn fetch_version_age_returns_age_from_cached_meta() {
        let dir = tempdir().unwrap();
        let config = Config {
            cache_dir: Some(dir.path().to_path_buf()),
            ..Config::default()
        };
        let resolver = Resolver::new(&config).unwrap();
        let expected_age = 10_080u64; // 7 days
        let created_at = *NOW - chrono::Duration::minutes(expected_age.try_into().unwrap());
        let meta = VersionMeta {
            num: "1.43.0".into(),
            created_at,
            yanked: false,
        };
        resolver.cache.put("tokio/1.43.0", &meta).unwrap();

        let age = resolver.fetch_version_age("tokio", "1.43.0").await.unwrap();

        assert_eq!(age, expected_age);
    }

    #[tokio::test]
    async fn find_version_candidates_returns_versions_satisfying_all_requirements() {
        let (resolver, _dir) = resolver_with_cached_tokio_versions();
        let requirements = vec![
            VersionReq::parse("^1").unwrap(),
            VersionReq::parse("^1.42").unwrap(),
            VersionReq::parse("^1.43").unwrap(),
        ];

        let candidates = resolver
            .find_version_candidates(&tokio_failure(), &requirements)
            .await
            .unwrap();

        let versions: Vec<String> = candidates.iter().map(ToString::to_string).collect();
        assert_eq!(versions, vec!["1.43.3", "1.43.0"]);
    }

    #[tokio::test]
    async fn find_version_candidates_excludes_yanked_versions() {
        let dir = tempdir().unwrap();
        let config = Config {
            cache_dir: Some(dir.path().to_path_buf()),
            ..Config::default()
        };
        let resolver = Resolver::new(&config).unwrap();
        let old = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let versions = vec![
            VersionMeta {
                num: "1.43.4".into(),
                created_at: old,
                yanked: false,
            },
            VersionMeta {
                num: "1.43.3".into(),
                created_at: old,
                yanked: true,
            },
            VersionMeta {
                num: "1.43.0".into(),
                created_at: old,
                yanked: false,
            },
        ];
        resolver.cache.put("tokio/_list", &versions).unwrap();

        let requirements = vec![VersionReq::parse("^1.43").unwrap()];
        let candidates = resolver
            .find_version_candidates(&tokio_failure(), &requirements)
            .await
            .unwrap();

        let versions: Vec<String> = candidates.iter().map(ToString::to_string).collect();
        assert_eq!(versions, vec!["1.43.0"]);
    }

    #[tokio::test]
    async fn find_version_candidates_excludes_versions_within_cooldown_period() {
        let dir = tempdir().unwrap();
        let config = Config {
            cache_dir: Some(dir.path().to_path_buf()),
            ..Config::default()
        };
        let resolver = Resolver::new(&config).unwrap();
        let old = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let fresh = Utc::now();
        let versions = vec![
            VersionMeta {
                num: "1.43.4".into(),
                created_at: fresh,
                yanked: false,
            },
            VersionMeta {
                num: "1.43.3".into(),
                created_at: fresh,
                yanked: false,
            },
            VersionMeta {
                num: "1.43.0".into(),
                created_at: old,
                yanked: false,
            },
        ];
        resolver.cache.put("tokio/_list", &versions).unwrap();

        let failure = CooldownFailure {
            package_id: PackageId {
                repr: "tokio 1.43.4".to_string(),
            },
            name: "tokio".to_string(),
            current_version: "1.43.4".to_string(),
            minimum_minutes: 10080,
        };
        let requirements = vec![VersionReq::parse("^1.43").unwrap()];
        let candidates = resolver
            .find_version_candidates(&failure, &requirements)
            .await
            .unwrap();

        let versions: Vec<String> = candidates.iter().map(ToString::to_string).collect();
        assert_eq!(versions, vec!["1.43.0"]);
    }

    #[tokio::test]
    async fn find_version_candidates_returns_empty_when_no_older_version_satisfies_requirements() {
        let (resolver, _dir) = resolver_with_cached_tokio_versions();
        let requirements = vec![VersionReq::parse("^1.43.4").unwrap()];

        let candidates = resolver
            .find_version_candidates(&tokio_failure(), &requirements)
            .await
            .unwrap();

        assert!(
            candidates.is_empty(),
            "expected no candidates, got {candidates:?}"
        );
    }
}
