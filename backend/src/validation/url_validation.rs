//! URL validation and SSRF protections for outbound monitoring requests.
//!
//! Three cooperating layers prevent a user-supplied monitor URL from being
//! used to probe internal infrastructure:
//!
//! 1. **Structure validation** ([`validate_monitor_url`]) at create/update time:
//!    http(s) only, no credentials, hostname required, blocklisted hostnames,
//!    and no private/loopback/link-local/etc. IP literals.
//! 2. **Connect-time DNS filtering** ([`SsrfDnsResolver`]): reqwest resolves
//!    every hostname through this resolver, which drops (or refuses) any
//!    address that is not publicly routable. Because reqwest only connects to
//!    addresses the resolver returned, DNS rebinding cannot bypass the check.
//! 3. **Redirect revalidation** ([`redirect_policy`]): every redirect hop is
//!    re-checked with the same structure rules (max 5 hops).
//!
//! The environment variable `PULSEWATCH_ALLOW_PRIVATE_TARGETS=true` (dev/tests
//! only) disables checks 1's private-IP/hostname rules and 2's filtering so a
//! local mock server on 127.0.0.1 is reachable. Production defaults block
//! everything private.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use url::{Host, Url};

use crate::config::Config;

/// Maximum accepted URL length (characters), including query string.
pub const MAX_URL_LENGTH: usize = 2048;
/// Maximum redirect hops followed for a single check.
pub const MAX_REDIRECT_HOPS: usize = 5;

/// Which destinations the outbound client is allowed to reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SsrfPolicy {
    /// Dev/test escape hatch: permits private, loopback and link-local
    /// destinations. Must stay `false` in production.
    pub allow_private: bool,
}

impl SsrfPolicy {
    pub fn from_config(config: &Config) -> Self {
        Self {
            allow_private: config.allow_private_targets,
        }
    }

    /// Policy that enforces every restriction (production default).
    pub fn strict() -> Self {
        Self {
            allow_private: false,
        }
    }
}

/// Why a URL was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UrlValidationError {
    #[error("must be a valid absolute URL")]
    InvalidUrl,
    #[error("URL is too long (max 2048 characters)")]
    TooLong,
    #[error("scheme must be http or https")]
    UnsupportedScheme,
    #[error("URL must not contain embedded credentials")]
    ContainsCredentials,
    #[error("URL must include a hostname")]
    MissingHost,
    #[error("hostname '{0}' is not allowed")]
    BlockedHost(String),
    #[error("hostname '{0}' must be a fully-qualified domain name")]
    UnqualifiedHostname(String),
    #[error("IP address {0} is not a publicly routable address")]
    PrivateAddress(IpAddr),
}

/// Hostnames that are never valid monitoring targets (cloud metadata and
/// special-use names). Checked case-insensitively against the exact name or
/// any parent-domain suffix.
const BLOCKED_HOSTNAMES: &[&str] = &["metadata.google.internal", "metadata.goog", "instance-data"];

/// Domain suffixes that only ever resolve inside a local network.
const BLOCKED_SUFFIXES: &[&str] = &[".local", ".internal", ".home.arpa", ".localhost"];

/// Validate a monitor URL and return its parsed form.
///
/// Enforces scheme, credential, hostname and (unless `policy.allow_private`)
/// public-IP rules. The *resolved* addresses are filtered separately by
/// [`SsrfDnsResolver`] at connect time.
pub fn validate_monitor_url(raw: &str, policy: &SsrfPolicy) -> Result<Url, UrlValidationError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(UrlValidationError::InvalidUrl);
    }
    if trimmed.len() > MAX_URL_LENGTH {
        return Err(UrlValidationError::TooLong);
    }

    let url = Url::parse(trimmed).map_err(|_| UrlValidationError::InvalidUrl)?;
    match url.scheme() {
        "http" | "https" => {}
        _ => return Err(UrlValidationError::UnsupportedScheme),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(UrlValidationError::ContainsCredentials);
    }

    let host = url.host().ok_or(UrlValidationError::MissingHost)?;
    match host {
        Host::Domain(domain) => {
            if !policy.allow_private {
                validate_domain(domain)?;
            }
        }
        Host::Ipv4(ip) => {
            let ip = IpAddr::V4(ip);
            if !policy.allow_private && !is_public_ip(ip) {
                return Err(UrlValidationError::PrivateAddress(ip));
            }
        }
        Host::Ipv6(ip) => {
            let ip = IpAddr::V6(ip);
            if !policy.allow_private && !is_public_ip(ip) {
                return Err(UrlValidationError::PrivateAddress(ip));
            }
        }
    }
    Ok(url)
}

/// Hostname rules: blocklist, special-use suffixes, and single-label names.
fn validate_domain(domain: &str) -> Result<(), UrlValidationError> {
    let lower = domain.to_ascii_lowercase();
    if lower == "localhost" || BLOCKED_HOSTNAMES.contains(&lower.as_str()) {
        return Err(UrlValidationError::BlockedHost(domain.to_string()));
    }
    if BLOCKED_SUFFIXES
        .iter()
        .any(|suffix| lower.ends_with(suffix))
    {
        return Err(UrlValidationError::BlockedHost(domain.to_string()));
    }
    // Single-label names ("intranet") resolve through search domains to
    // internal hosts; require at least one dot.
    if !lower.contains('.') {
        return Err(UrlValidationError::UnqualifiedHostname(domain.to_string()));
    }
    Ok(())
}

/// Whether an address may be contacted: rejects private, loopback, link-local,
/// CGNAT, multicast, unspecified, documentation, benchmarking and transition
/// (6to4/Teredo/NAT64/embedded-IPv4) ranges for both IPv4 and IPv6.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    let blocked = match a {
        // 0.0.0.0/8 "this network"
        0 => true,
        // 10.0.0.0/8 private
        10 => true,
        // 100.64.0.0/10 carrier-grade NAT
        100 => (b & 0xC0) == 64,
        // 127.0.0.0/8 loopback
        127 => true,
        // 169.254.0.0/16 link-local (cloud metadata lives here)
        169 => b == 254,
        // 172.16.0.0/12 private
        172 => (b & 0xF0) == 16,
        // 192.0.0.0/24 IETF protocol assignments, 192.0.2.0/24 TEST-NET-1
        192 => (b == 0 && (c == 0 || c == 2)) || b == 168,
        // 198.18.0.0/15 benchmarking, 198.51.100.0/24 TEST-NET-2
        198 => (b & 0xFE) == 18 || (b == 51 && c == 100),
        // 203.0.113.0/24 TEST-NET-3
        203 => b == 0 && c == 113,
        // 224.0.0.0/4 multicast, 240.0.0.0/4 reserved, broadcast
        _ => a >= 224,
    };
    !blocked
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let seg = ip.segments();
    if ip.is_unspecified() || ip.is_loopback() || ip.is_multicast() {
        return false;
    }
    // IPv4-mapped (::ffff:0:0/96): judge the embedded IPv4 address.
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    // IPv4-compatible (::a.b.c.d, deprecated): first 6 segments are zero.
    if seg[..6].iter().all(|s| *s == 0) {
        return false;
    }
    // 6to4 2002::/16 and Teredo 2001:0000::/32 embed attacker-chosen IPv4.
    if seg[0] == 0x2002 || (seg[0] == 0x2001 && seg[1] == 0x0000) {
        return false;
    }
    // 2001:db8::/32 documentation.
    if seg[0] == 0x2001 && seg[1] == 0x0db8 {
        return false;
    }
    // 0100::/64 discard-only.
    if seg[0] == 0x0100 && seg[1..4].iter().all(|s| *s == 0) {
        return false;
    }
    // 64:ff9b::/96 NAT64 (embedded IPv4).
    if seg[0] == 0x0064 && seg[1] == 0xff9b {
        return false;
    }
    // fc00::/7 unique-local.
    if seg[0] & 0xfe00 == 0xfc00 {
        return false;
    }
    // fe80::/10 link-local, fec0::/10 deprecated site-local.
    if seg[0] & 0xffc0 == 0xfe80 || seg[0] & 0xffc0 == 0xfec0 {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Marker errors: carried inside reqwest errors so the check executor can tell
// *why* a redirect/connection failed (reqwest erases the cause into a generic
// error kind). Both must be `Send + Sync + 'static` to be boxed as causes.
// ---------------------------------------------------------------------------

/// Marker for a destination that failed the SSRF policy (hop or resolved IP).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedTarget(pub String);

impl fmt::Display for BlockedTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "blocked target: {}", self.0)
    }
}

impl std::error::Error for BlockedTarget {}

/// Marker for exceeding [`MAX_REDIRECT_HOPS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooManyRedirects;

impl fmt::Display for TooManyRedirects {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "too many redirects (max {MAX_REDIRECT_HOPS})")
    }
}

impl std::error::Error for TooManyRedirects {}

/// Walk an error's `source()` chain looking for a concrete marker type.
pub(crate) fn chain_contains<T>(error: &(dyn std::error::Error + 'static)) -> bool
where
    T: std::error::Error + 'static,
{
    let mut current = Some(error);
    while let Some(err) = current {
        if err.is::<T>() {
            return true;
        }
        current = err.source();
    }
    false
}

// ---------------------------------------------------------------------------
// DNS resolver + redirect policy + client construction
// ---------------------------------------------------------------------------

/// reqwest DNS resolver that drops every non-public address.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SsrfDnsResolver {
    pub(crate) policy: SsrfPolicy,
}

impl reqwest::dns::Resolve for SsrfDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let allow_private = self.policy.allow_private;
        let host = name.as_str().to_string();
        Box::pin(async move {
            type BoxError = Box<dyn std::error::Error + Send + Sync>;

            let addresses = tokio::net::lookup_host((host.as_str(), 0))
                .await
                .map_err(|err| Box::new(err) as BoxError)?;

            let mut seen = false;
            let mut allowed: Vec<std::net::SocketAddr> = Vec::new();
            for addr in addresses {
                seen = true;
                if allow_private || is_public_ip(addr.ip()) {
                    allowed.push(std::net::SocketAddr::new(addr.ip(), 0));
                }
            }

            if !seen {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "hostname resolved to no addresses",
                )) as BoxError);
            }
            if allowed.is_empty() {
                return Err(Box::new(BlockedTarget(format!(
                    "'{host}' resolves only to non-public addresses"
                ))) as BoxError);
            }
            Ok(Box::new(allowed.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

/// Redirect policy: bounded hop count plus per-hop destination revalidation.
pub(crate) fn redirect_policy(policy: SsrfPolicy) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() > MAX_REDIRECT_HOPS {
            return attempt.error(TooManyRedirects);
        }
        if let Err(err) = validate_monitor_url(attempt.url().as_str(), &policy) {
            return attempt.error(BlockedTarget(format!("redirect target rejected: {err}")));
        }
        attempt.follow()
    })
}

/// Build the shared outbound HTTP client.
///
/// Proxies are disabled (`system-proxy` feature off + `.no_proxy()`) so an
/// environment proxy can never route a request around the destination checks.
pub fn build_safe_http_client(config: &Config) -> Result<reqwest::Client, reqwest::Error> {
    let policy = SsrfPolicy::from_config(config);
    reqwest::Client::builder()
        .user_agent(concat!("PulseWatch/", env!("CARGO_PKG_VERSION")))
        .redirect(redirect_policy(policy))
        .dns_resolver(SsrfDnsResolver { policy })
        .no_proxy()
        .connect_timeout(Duration::from_secs(10))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strict(raw: &str) -> Result<Url, UrlValidationError> {
        validate_monitor_url(raw, &SsrfPolicy::strict())
    }

    fn permissive(raw: &str) -> Result<Url, UrlValidationError> {
        validate_monitor_url(
            raw,
            &SsrfPolicy {
                allow_private: true,
            },
        )
    }

    #[test]
    fn accepts_public_http_and_https_urls() {
        assert!(strict("https://example.com").is_ok());
        assert!(strict("http://example.com/health?full=1").is_ok());
        assert!(strict("https://sub.domain.example.org:8443/path").is_ok());
        assert!(strict("https://8.8.8.8/").is_ok());
    }

    #[test]
    fn rejects_invalid_urls() {
        assert_eq!(strict(""), Err(UrlValidationError::InvalidUrl));
        assert_eq!(strict("not a url"), Err(UrlValidationError::InvalidUrl));
        assert_eq!(
            strict("ftp://example.com"),
            Err(UrlValidationError::UnsupportedScheme)
        );
        assert_eq!(
            strict("file:///etc/passwd"),
            Err(UrlValidationError::UnsupportedScheme)
        );
        assert_eq!(
            strict("http://user:pass@example.com"),
            Err(UrlValidationError::ContainsCredentials)
        );
        assert_eq!(
            strict(&"http://example.com/".repeat(200)),
            Err(UrlValidationError::TooLong)
        );
    }

    #[test]
    fn rejects_private_and_special_ipv4_literals() {
        for url in [
            "http://127.0.0.1/",
            "http://127.1.2.3:8080/",
            "http://10.0.0.5/",
            "http://172.16.0.1/",
            "http://172.31.255.255/",
            "http://192.168.1.1/",
            "http://169.254.169.254/latest/meta-data/",
            "http://100.64.0.1/",
            "http://0.0.0.0/",
            "http://255.255.255.255/",
            "http://224.0.0.1/",
            "http://198.18.0.1/",
            "http://192.0.2.10/",
            "http://198.51.100.10/",
            "http://203.0.113.10/",
        ] {
            match strict(url) {
                Err(UrlValidationError::PrivateAddress(_)) => {}
                other => panic!("{url} should be rejected as private, got {other:?}"),
            }
        }
    }

    #[test]
    fn rejects_private_ipv6_literals() {
        for url in [
            "http://[::1]/",
            "http://[::]/",
            "http://[fe80::1]/",
            "http://[fc00::1]/",
            "http://[fd12:3456:789a::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://[::ffff:10.0.0.1]/",
            "http://[64:ff9b::7f00:1]/",
            "http://[2002:7f00:1::]/",
            "http://[2001:db8::1]/",
            "http://[2001::1]/",
            "http://[ff02::1]/",
        ] {
            match strict(url) {
                Err(UrlValidationError::PrivateAddress(_)) => {}
                other => panic!("{url} should be rejected as private, got {other:?}"),
            }
        }
    }

    #[test]
    fn rejects_embedded_ipv6_but_allows_mapped_public() {
        // IPv4-mapped public addresses are legitimate.
        assert!(strict("http://[::ffff:8.8.8.8]/").is_ok());
        // Global unicast v6 is fine.
        assert!(strict("http://[2606:4700:4700::1111]/").is_ok());
    }

    #[test]
    fn rejects_blocklisted_and_unqualified_hostnames() {
        for url in [
            "http://localhost/",
            "http://LOCALHOST:3000/",
            "http://foo.localhost/",
            "http://printer.local/",
            "http://db.internal/",
            "http://router.home.arpa/",
            "http://metadata.google.internal/",
            "http://METADATA.GOOG/",
            "http://intranet/",
        ] {
            assert!(
                matches!(
                    strict(url),
                    Err(UrlValidationError::BlockedHost(_))
                        | Err(UrlValidationError::UnqualifiedHostname(_))
                ),
                "{url} should be rejected, got {:?}",
                strict(url)
            );
        }
    }

    #[test]
    fn allow_private_permits_local_targets() {
        assert!(permissive("http://127.0.0.1:8080/").is_ok());
        assert!(permissive("http://localhost:8080/").is_ok());
        assert!(permissive("http://[::1]:8080/").is_ok());
        // Structural rules still apply.
        assert!(permissive("ftp://127.0.0.1/").is_err());
        assert!(permissive("http://user:pass@127.0.0.1/").is_err());
    }

    #[test]
    fn is_public_ip_matrix() {
        let public = [
            "1.1.1.1",
            "8.8.8.8",
            "93.184.216.34",
            "172.15.255.255", // just outside 172.16/12
            "172.32.0.1",     // just outside
            "2606:4700:4700::1111",
            "::ffff:8.8.8.8",
        ];
        for ip in public {
            assert!(is_public_ip(ip.parse().unwrap()), "{ip} should be public");
        }

        let private = [
            "0.1.2.3",
            "10.1.2.3",
            "100.64.0.1",
            "100.127.255.255",
            "127.0.0.1",
            "169.254.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.0.0.1",
            "192.0.2.1",
            "192.168.0.1",
            "198.18.0.1",
            "198.19.255.255",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "::ffff:127.0.0.1",
            "::a00:1", // IPv4-compatible 0.0.0.10.0.1
            "fe80::1",
            "fc00::1",
            "fd00::1",
            "fec0::1",
            "ff02::1",
            "64:ff9b::808:808",
            "2002:808:808::",
            "2001::1",
            "2001:db8::1",
            "100::1",
        ];
        for ip in private {
            assert!(
                !is_public_ip(ip.parse().unwrap()),
                "{ip} should NOT be public"
            );
        }
    }

    #[test]
    fn marker_errors_survive_error_chains() {
        #[derive(Debug)]
        struct Wrapper(BlockedTarget);
        impl fmt::Display for Wrapper {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "outer: {}", self.0)
            }
        }
        impl std::error::Error for Wrapper {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }

        let wrapped = Wrapper(BlockedTarget("nope".into()));
        assert!(chain_contains::<BlockedTarget>(&wrapped));
        assert!(!chain_contains::<TooManyRedirects>(&wrapped));
    }

    #[tokio::test]
    async fn dns_resolver_blocks_private_only_hosts() {
        use reqwest::dns::Resolve;

        // "localhost" always resolves locally (/etc/hosts), never the network.
        let strict = SsrfDnsResolver {
            policy: SsrfPolicy::strict(),
        };
        let name: reqwest::dns::Name = "localhost".parse().unwrap();
        let result = strict.resolve(name).await;
        let err = match result {
            Ok(_) => panic!("localhost must be refused by the strict resolver"),
            Err(err) => err,
        };
        assert!(chain_contains::<BlockedTarget>(&*err));

        let permissive = SsrfDnsResolver {
            policy: SsrfPolicy {
                allow_private: true,
            },
        };
        let name: reqwest::dns::Name = "localhost".parse().unwrap();
        let addrs = permissive
            .resolve(name)
            .await
            .expect("localhost must resolve in permissive mode");
        assert!(addrs.count() > 0);
    }
}
