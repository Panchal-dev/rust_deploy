//! Browser / solver manager.
//!
//! Wraps `chaser-cf`'s `ChaserCF` and exposes a small, stable interface used
//! by the endpoint handlers. If the underlying solver is swapped later, only
//! this file needs to change.

use std::sync::Arc;
use std::time::Instant;

use chaser_cf::{ChaserCF, ChaserConfig, ProxyConfig};

use crate::config::Config;
use crate::proxy::NormalizedProxy;

/// Shared solver handle. Internally reference-counted; clone is cheap.
pub struct BrowserManager {
    chaser: Arc<ChaserCF>,
    timeout_ms: u64,
    started_at: Instant,
    context_limit: usize,
}

impl BrowserManager {
    /// Build the solver once at startup. This launches Xvfb (on Linux) and a
    /// warm browser context pool, so subsequent solves are fast.
    pub async fn new(config: &Config) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let chaser_config = ChaserConfig::default()
            .with_virtual_display(cfg!(target_os = "linux"))
            .with_context_limit(config.browser_limit)
            .with_timeout_ms(config.timeout_ms)
            .add_extra_arg("--no-sandbox")
            .add_extra_arg("--disable-dev-shm-usage");

        let chaser = ChaserCF::new(chaser_config)
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("ChaserCF init failed: {e}").into()
            })?;

        Ok(Self {
            chaser: Arc::new(chaser),
            timeout_ms: config.timeout_ms,
            started_at: Instant::now(),
            context_limit: config.browser_limit,
        })
    }

    /// Seconds since the solver was created (approximate uptime).
    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Reported pool size (used as a rough "contexts available" metric).
    pub fn contexts_available(&self) -> usize {
        self.context_limit
    }

    /// Solve an IUAM (managed) challenge, returning the `cf_clearance` cookie
    /// plus the associated user agent and cookie jar.
    pub async fn solve_iuam(
        &self,
        domain: &str,
        proxy: Option<&NormalizedProxy>,
    ) -> Result<IuamOutcome, Box<dyn std::error::Error + Send + Sync>> {
        let proxy_cfg = build_proxy_config(proxy);

        let session = tokio::time::timeout(
            std::time::Duration::from_millis(self.timeout_ms),
            self.chaser.solve_waf_session(domain, proxy_cfg),
        )
        .await
        .map_err(|_| -> Box<dyn std::error::Error + Send + Sync> {
            "solve timed out".into()
        })??;

        // WafSession exposes `cookies: Vec<Cookie>` and
        // `headers: HashMap<String, String>`. Extract cf_clearance and
        // user-agent from those.
        let cf_clearance = session
            .cookies
            .iter()
            .find(|c| c.name == "cf_clearance")
            .map(|c| c.value.clone())
            .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                "cf_clearance cookie not present in session".into()
            })?;

        let user_agent = session
            .headers
            .get("user-agent")
            .or_else(|| session.headers.get("User-Agent"))
            .cloned()
            .unwrap_or_default();

        let cookies: Vec<String> = session
            .cookies
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect();

        Ok(IuamOutcome {
            cf_clearance,
            user_agent,
            cookies,
        })
    }

    /// Solve a Turnstile widget with a known site key. Returns the token.
    pub async fn solve_turnstile(
        &self,
        domain: &str,
        site_key: &str,
        proxy: Option<&NormalizedProxy>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let proxy_cfg = build_proxy_config(proxy);

        let token = tokio::time::timeout(
            std::time::Duration::from_millis(self.timeout_ms),
            self.chaser
                .solve_turnstile_min(domain, site_key, proxy_cfg),
        )
        .await
        .map_err(|_| -> Box<dyn std::error::Error + Send + Sync> {
            "turnstile solve timed out".into()
        })??;

        Ok(token)
    }
}

/// Result of an IUAM solve.
pub struct IuamOutcome {
    pub cf_clearance: String,
    pub user_agent: String,
    pub cookies: Vec<String>,
}

/// Build an optional `ProxyConfig` from our normalised proxy.
///
/// `chaser-cf` 0.2.x takes the proxy as an explicit argument on every solve
/// call, rather than as a builder on the solver itself. The `with_auth`
/// method requires both username and password — we only call it when both
/// are present.
fn build_proxy_config(proxy: Option<&NormalizedProxy>) -> Option<ProxyConfig> {
    let p = proxy?;

    let mut cfg = ProxyConfig::new(&p.host, p.port).with_scheme(p.chrome_scheme());

    if let (Some(user), Some(pass)) = (p.username.as_deref(), p.password.as_deref()) {
        cfg = cfg.with_auth(user, pass);
    }

    Some(cfg)
}