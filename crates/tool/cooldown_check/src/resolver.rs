use std::sync::LazyLock;

use chrono::{DateTime, Utc};

use crate::registry::VersionMeta;

// Create the static singleton instance
static NOW: LazyLock<DateTime<Utc>> = LazyLock::new(|| {
    // This closure runs only once, the first time NOW is accessed
    Utc::now()
});

pub fn age_minutes(datetime: DateTime<Utc>) -> i64 {
    (*NOW - datetime).num_minutes()
}

pub fn filter_candidates(versions: Vec<VersionMeta>, minimum_minutes: u64) -> Vec<VersionMeta> {
    filter_candidates_by_time(versions, minimum_minutes, *NOW)
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
