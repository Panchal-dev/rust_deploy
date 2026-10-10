//! Bounded TTL cache wrapping `moka`.

use crate::models::BypassResponse;
use moka::future::Cache;
use std::time::Duration;

/// Cache used for non-proxied IUAM results (they are IP-scoped, so we only
/// store the ones tied to this server's egress IP).
///
/// A single global TTL of 30 minutes is applied at construction. Per-entry
/// TTL overrides are not supported by moka 0.12's simple insert API; the
/// global ceiling is sufficient for our use-case.
#[derive(Clone)]
pub struct TtlCache {
    inner: Cache<String, BypassResponse>,
}

impl TtlCache {
    /// Create a new cache with a hard cap on entry count and a global TTL of
    /// 30 minutes.
    pub fn new(max_entries: u64) -> Self {
        Self {
            inner: Cache::builder()
                .max_capacity(max_entries)
                .time_to_live(Duration::from_secs(30 * 60))
                .build(),
        }
    }

    /// Return a cached response if present and unexpired.
    pub async fn get(&self, key: &str) -> Option<BypassResponse> {
        self.inner.get(key).await
    }

    /// Insert a response using the cache's global TTL.
    pub async fn set(&self, key: String, value: BypassResponse) {
        self.inner.insert(key, value).await;
    }
}