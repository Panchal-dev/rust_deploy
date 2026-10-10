//! Wire types for the HTTP API.

use serde::{Deserialize, Serialize};

/// Incoming /cloudflare request body.
#[derive(Debug, Clone, Deserialize)]
pub struct BypassRequest {
    /// "iuam" or "turnstile".
    pub mode: String,
    /// Fully-qualified target URL (e.g. "https://example.com").
    pub domain: String,
    /// Required for turnstile mode.
    #[serde(default)]
    pub site_key: Option<String>,
    /// Optional proxy. String or object — see `proxy.rs`.
    #[serde(default)]
    pub proxy: Option<crate::proxy::ProxyInput>,
    /// Optional cache TTL override in milliseconds.
    #[serde(default)]
    pub ttl: Option<u64>,
    /// Shared secret sent when AUTH_TOKEN is configured on the server.
    #[serde(default, rename = "authToken")]
    pub auth_token: Option<String>,
}

/// Outgoing /cloudflare response body.
#[derive(Debug, Clone, Serialize)]
pub struct BypassResponse {
    pub code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cf_clearance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookies: Option<Vec<String>>,
    pub elapsed: String,
    #[serde(default)]
    pub cached: bool,
}

impl BypassResponse {
    /// Base success response with `code = 200`; callers fill in the payload.
    pub fn success_iuam(cf_clearance: String, user_agent: String, cookies: Vec<String>) -> Self {
        Self {
            code: 200,
            cf_clearance: Some(cf_clearance),
            user_agent: Some(user_agent),
            token: None,
            cookies: Some(cookies),
            elapsed: String::new(),
            cached: false,
        }
    }

    pub fn success_turnstile(token: String) -> Self {
        Self {
            code: 200,
            cf_clearance: None,
            user_agent: None,
            token: Some(token),
            cookies: None,
            elapsed: String::new(),
            cached: false,
        }
    }
}

/// Health probe response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub uptime_seconds: u64,
    pub contexts_available: usize,
    pub auth_required: bool,
}
