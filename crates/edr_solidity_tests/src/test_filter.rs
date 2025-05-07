use std::path::Path;

use regex::Regex;

/// Test filter.
pub trait TestFilter: Send + Sync {
    /// Returns whether the test should be included.
    fn matches_test(&self, test_name: &str) -> bool;

    /// Returns whether the contract should be included.
    fn matches_contract(&self, contract_name: &str) -> bool;

    /// Returns a contract with the given path should be included.
    fn matches_path(&self, path: &Path) -> bool;
}

pub struct TestFilterConfig {
    pub test_pattern: Option<Regex>,
}

impl TestFilter for TestFilterConfig {
    fn matches_test(&self, test_name: &str) -> bool {
        self.test_pattern
            .as_ref()
            .is_none_or(|p| p.is_match(test_name))
    }

    fn matches_contract(&self, _contract_name: &str) -> bool {
        true
    }

    fn matches_path(&self, _path: &Path) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_none() {
        let config = TestFilterConfig { test_pattern: None };

        assert!(config.matches_test("test_foo"));
        assert!(config.matches_test("test_bar"));
    }

    #[test]
    fn test_pattern_some() {
        let config = TestFilterConfig {
            test_pattern: Some("f?o+".parse().unwrap()),
        };

        assert!(config.matches_test("test_foo"));
        assert!(!config.matches_test("test_bar"));
    }
}
