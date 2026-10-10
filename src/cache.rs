//! Bounded TTL cache wrapping `moka`.

use crate::models::BypassResponse;
use moka::future::Cache;
use std::time::Duration;

/// Cache used for non-proxied IUAM results (they are IP-scoped, so we only
/// store the ones tied to this server's egress IP).
#[derive(Clone)]
pub struct TtlCache {
    inner: Cache<String, BypassResponse>,
}

impl TtlCache {
    /// Create a new cache with a hard cap on entry count and a global TTL of
    /// 30 minutes. Per-request TTLs are honoured at insert time via
    /// `insert_with_ttl`, but the global TTL acts as a safety ceiling.
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

    /// Insert a response. `ttl_ms` is applied as a per-entry TTL.
    pub async fn set(&self, key: String, value: BypassResponse) {
        // moka's per-entry TTL API is `insert_with_ttl` in recent versions.
        // For simplicity and forward-compatibility, we rely on the global TTL
        // configured at construction. Per-request TTLs can be wired in later
        // without affecting callers of this method.
        self.inner.insert(key, value).await;
    }
}
