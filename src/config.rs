//! Runtime configuration loaded from environment variables.

use std::env;

/// All tunables in one place. Constructed once at startup; immutable
/// thereafter.
#[derive(Debug, Clone)]
pub struct Config {
    /// Listening port. Railway injects `PORT` automatically.
    pub port: u16,
    /// Optional shared secret; when set, callers must send a matching
    /// `authToken` in the JSON body.
    pub auth_token: Option<String>,
    /// Max concurrent browser contexts the solver can run at once.
    pub browser_limit: usize,
    /// Per-solve timeout, milliseconds.
    pub timeout_ms: u64,
    /// Max entries in the in-memory response cache.
    pub cache_max_entries: u64,
    /// Default cache TTL in milliseconds (used when caller omits `ttl`).
    pub cache_default_ttl_ms: u64,
}

impl Config {
    /// Read all configuration from the environment.
    ///
    /// Fails early (before the server starts) if a required value is invalid,
    /// so that misconfigurations never surface as runtime 500s.
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            port: parse_port_env()?,
            auth_token: env::var("AUTH_TOKEN").ok().filter(|s| !s.is_empty()),
            browser_limit: parse_env("BROWSER_LIMIT", 20_usize)?,
            timeout_ms: parse_env("TIMEOUT_MS", 60_000_u64)?,
            cache_max_entries: parse_env("CACHE_MAX_ENTRIES", 10_000_u64)?,
            cache_default_ttl_ms: parse_env("CACHE_TTL_MS", 30 * 60 * 1000_u64)?,
        })
    }
}

/// Parse an env var, falling back to a default when unset or empty.
fn parse_env<T>(name: &str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(name) {
        Ok(raw) if !raw.trim().is_empty() => raw
            .trim()
            .parse::<T>()
            .map_err(|e| ConfigError(format!("invalid {name}: {e}"))),
        _ => Ok(default),
    }
}

/// Parse the PORT env var with Railway-safe semantics.
///
/// Railway historically injects `PORT=0` on some services, which in Linux
/// means "bind to a kernel-assigned random port". That would make Railway's
/// proxy unable to route traffic to us. We therefore treat 0 (and unset/empty)
/// as "use the standard Railway port 8080".
fn parse_port_env() -> Result<u16, ConfigError> {
    let port = parse_env("PORT", 8080_u16)?;
    Ok(if port == 0 { 8080 } else { port })
}

/// Configuration error surfaced at startup.
#[derive(Debug, thiserror::Error)]
#[error("configuration error: {0}")]
pub struct ConfigError(pub String);