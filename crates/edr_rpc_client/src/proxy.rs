//! Proxy-related helpers for constructing HTTP clients.

use url::{Host, Url};

/// Whether `url` points at the loopback interface.
///
/// `reqwest` honours `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY` for every request
/// by default and only exempts hosts listed in `NO_PROXY`. Clients targeting a
/// loopback URL are built without proxy support.
///
/// See <https://github.com/NomicFoundation/edr/issues/1762>.
pub fn is_loopback_url(url: &Url) -> bool {
    match url.host() {
        // `Url` lower-cases domain names, so no case folding is needed.
        Some(Host::Domain(domain)) => domain == "localhost" || domain.ends_with(".localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback() || address.is_unspecified(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(url: &str) -> bool {
        is_loopback_url(&url.parse().expect("valid url"))
    }

    #[test]
    fn loopback_urls() {
        for url in [
            "http://localhost",
            "http://LOCALHOST:8545",
            "http://api.localhost:8545/path",
            "http://127.0.0.1:8545",
            "http://127.1.2.3",
            "http://[::1]:8545",
            "http://0.0.0.0:8545",
            "https://localhost:8545",
        ] {
            assert!(check(url), "{url} should be loopback");
        }
    }

    #[test]
    fn non_loopback_urls() {
        for url in [
            "https://mainnet.infura.io/v3/key",
            "http://192.168.1.10:8545",
            "http://notlocalhost.com",
            "http://localhost.example.com",
            "http://[2001:db8::1]:8545",
            "http://10.0.0.1",
        ] {
            assert!(!check(url), "{url} should not be loopback");
        }
    }
}
