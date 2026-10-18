//! Input validation and outbound-request safety.

pub mod url_validation;

pub(crate) use url_validation::chain_contains;
pub use url_validation::{
    build_safe_http_client, is_public_ip, validate_monitor_url, BlockedTarget, SsrfPolicy,
    TooManyRedirects, UrlValidationError,
};
